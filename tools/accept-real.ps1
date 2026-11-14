#!/usr/bin/env pwsh
<#
  accept-real.ps1 -- real-machine multi-open acceptance for DSH-NEXT Multi-Launcher.

  Why this script exists:
    tools/e2e.mjs proves the launch pipeline against a stub process, and it must stay
    inside the workspace (it redirects data dir / versions / pointer). What it cannot
    prove is the part that only the real thing exercises: the REAL global pointer under
    %APPDATA%\DSH NEXT\home, the REAL DSH-NEXT shell, real windows, real taskkill.

  Where to run it:
    A normal user terminal. NOT from a restricted/sandboxed session: writing the global
    pointer outside the workspace is denied there and every launch fails with E1001
    ("access is denied"), which looks like a product bug but is the session's confinement.

  What it automates (PASS/FAIL, exit code 1 on failure):
    A0  --selftest green, pointer writable
    A1  instance A launches, a process appears for A's home, pointer restored to default
    A2  instance B launches, both homes running at the same time
    A3  second launch of A is rejected with E1003 and the pointer is untouched
    A4  real DSH-NEXT did not show "data directory differs" (proxy: last-home == pointer
        after each launch -- see note below)
    A5  graceful stop of A, forced stop of B, no leftover process for either home
    A6  cleanup: test instances and their homes removed, pointer back to its original text

  What stays manual (printed at the end):
    - two real windows on screen, each with its own home
    - no "data directory differs" modal on the second launch
    - safe-mode badge / restart menu (needs UI clicks)

  Usage:
    pwsh -NoProfile -File tools/accept-real.ps1 [-Exe .\DSH-NEXT-Multi.exe] [-Keep]

  Notes:
    - Test instances are written into the REAL launcher data dir and deleted afterwards.
      Instance ids are prefixed with "accept-" so a crash is easy to clean up by hand.
    - Homes live under %TEMP%\dsh-nul-accept\homes\<id> and are removed afterwards.
#>

param(
  [string]$Exe = (Join-Path $PSScriptRoot '..\DSH-NEXT-Multi.exe'),
  # Scratch root for test homes + CLI reports. Put it on a roomy drive if %TEMP% is small.
  [string]$WorkDir = (Join-Path $env:TEMP 'dsh-nul-accept'),
  [switch]$Keep
)

$ErrorActionPreference = 'Stop'
$script:PASS = 0
$script:FAIL = 0
$failures = New-Object System.Collections.Generic.List[string]

function Write-Check([string]$name, [bool]$ok, [string]$detail) {
  if ($ok) { $script:PASS++; Write-Host ("PASS  {0}{1}" -f $name, $(if ($detail) { " -- $detail" } else { '' })) }
  else { $script:FAIL++; $failures.Add($name); Write-Host ("FAIL  {0}{1}" -f $name, $(if ($detail) { " -- $detail" } else { '' })) -ForegroundColor Red }
}

function Get-CliJson([string]$outFile) {
  if (-not (Test-Path $outFile)) { return $null }
  try { return Get-Content $outFile -Raw | ConvertFrom-Json } catch { return $null }
}

function Invoke-Cli([string[]]$cliArgs, [string]$outFile) {
  if (Test-Path $outFile) { Remove-Item $outFile -Force }
  $env:DSH_MULTI_CLI_OUT = $outFile
  # The release exe is a GUI-subsystem binary: `& exe` does not wait and $LASTEXITCODE stays
  # stale, which made this script read a report file the launcher had not written yet.
  $p = Start-Process -FilePath $Exe -ArgumentList $cliArgs -NoNewWindow -Wait -PassThru
  return $p.ExitCode
}

function Get-RunningHomes($status) {
  if ($null -eq $status) { return @() }
  return @($status.running | ForEach-Object { $_.home })
}

function Normalize([string]$p) {
  if (-not $p) { return '' }
  return ($p -replace '/', '\').TrimEnd('\').ToLowerInvariant()
}

# ------------------------------------------------------------------ setup

$exePath = (Resolve-Path $Exe).Path
if (-not (Test-Path $exePath)) { Write-Error "exe not found: $Exe"; exit 2 }

# Data dir / versions dir are whatever the product resolves (no overrides) -- that is the point.
# The data dir is NOT always <Documents>\DSH-NEXT-Multi: it falls back to the exe-side portable
# dir when the Documents one is not writable, so take the resolved value from --selftest below.
$work = $WorkDir
$homesDir = Join-Path $work 'homes'
$cliOut = Join-Path $work 'cli.json'
$pointerPath = Join-Path $env:APPDATA 'DSH NEXT\home\desktop-next-location.json'
$lastHomePath = Join-Path $env:APPDATA 'DSH NEXT\home\desktop-next-last-home.json'

foreach ($envName in 'DSH_MULTI_DATA_DIR', 'DSH_MULTI_VERSIONS', 'DSH_MULTI_POINTER', 'DSH_MULTI_NO_ELEVATE', 'DSH_MULTI_NO_DIALOG') {
  if (Test-Path "env:$envName") { Remove-Item "env:$envName" }
}

New-Item -ItemType Directory -Force -Path $homesDir | Out-Null
$pointerBefore = if (Test-Path $pointerPath) { Get-Content $pointerPath -Raw } else { $null }

Write-Host ("== target      {0}" -f $exePath)
Write-Host ("== pointer     {0}" -f $pointerPath)
if ($pointerBefore) { Write-Host ("== pointer now {0}" -f ($pointerBefore -replace '\s+', ' ')) }

# ------------------------------------------------------------------ A0 selftest

$null = Invoke-Cli @('--selftest') $cliOut
$self = Get-CliJson $cliOut
if ($null -eq $self) {
  Write-Host ''
  Write-Host ("A0 cannot read the CLI report at {0}" -f $cliOut) -ForegroundColor Red
  Write-Host '  The launcher writes it under the scratch dir. If nothing appeared, the process could' -ForegroundColor Red
  Write-Host '  not write there at all -- run this script from a normal user terminal (not a sandboxed' -ForegroundColor Red
  Write-Host '  session): the global pointer lives outside the workspace and is denied there.' -ForegroundColor Red
  exit 1
}
$pathsCheck = $self.checks | Where-Object { $_.name -eq 'paths' }
$pointerCheck = $self.checks | Where-Object { $_.name -eq 'pointer' }
$versionCheck = $self.checks | Where-Object { $_.name -eq 'versions' }
$instanceCheck = $self.checks | Where-Object { $_.name -eq 'instances' }
$writeCheck = $self.checks | Where-Object { $_.name -eq 'atomicWrite' }
$dataDir = $pathsCheck.detail.dataDir
Write-Host ("== data dir    {0}" -f $dataDir)

# Gate only on what a launch actually needs. `--selftest ok` is deliberately NOT used as the
# gate: it also validates EVERY instance against the version tree in use, so running this
# script with the repo binary against your real data dir fails on unrelated instances whose
# version tag belongs to the installed launcher. That is environment state, not a launch
# blocker, so it is reported as INFO below instead of aborting the run.
$blocking = @(
  @{ name = 'paths'; ok = [bool]$pathsCheck.ok },
  @{ name = 'config'; ok = [bool]($self.checks | Where-Object { $_.name -eq 'config' }).ok },
  @{ name = 'versions'; ok = [bool]$versionCheck.ok },
  @{ name = 'atomicWrite'; ok = [bool]$writeCheck.ok },
  @{ name = 'pointer writable'; ok = [bool]$pointerCheck.detail.writable }
)
foreach ($b in $blocking) { Write-Check ("A0 {0}" -f $b.name) $b.ok '' }

$validVersions = @($versionCheck.detail | Where-Object { $_.valid })
Write-Check 'A0 at least one valid version' ($validVersions.Count -gt 0) (($validVersions | ForEach-Object { "$($_.tag) shell=$($_.shell)" }) -join ', ')

# Stale registrations (link name != version inside app.asar) are called out, not failed:
# the GUI reconciles them on startup, this CLI does not.
foreach ($v in $validVersions) {
  if ($v.shell -ne '?' -and $v.tag -ne $v.shell) {
    Write-Host ("INFO  A0 版本登记过期：versions\{0} 的安装树实际是 {1}（下次用 GUI 启动会改名对账）" -f $v.tag, $v.shell) -ForegroundColor Yellow
  }
}
if ($instanceCheck -and -not $instanceCheck.ok) {
  $invalid = @($instanceCheck.detail.invalid)
  Write-Host ("INFO  A0 {0} 个实例在当前版本树下无效（例如 {1}）-- 与本次验收无关" -f $invalid.Count, $invalid[0].id) -ForegroundColor Yellow
}

if ($script:FAIL -gt 0) {
  Write-Host ''
  Write-Host 'Launch prerequisites failed -- fix the environment before continuing (docs/guide/troubleshooting.md).'
  if (-not $Keep) { Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue }
  exit 1
}

$instancesDir = Join-Path $dataDir 'instances'
New-Item -ItemType Directory -Force -Path $instancesDir | Out-Null
# Prefer a registration whose tag matches the version inside app.asar; a stale link still
# launches fine, but the tag is what instances reference, so prefer the reconciled one.
$current = @($validVersions | Where-Object { $_.shell -ne '?' -and $_.tag -eq $_.shell })
$tag = if ($current.Count -gt 0) { $current[0].tag } else { $validVersions[0].tag }
$defaultHome = $self.checks | Where-Object { $_.name -eq 'config' } | ForEach-Object { $_.detail.defaultHome }

$instA = 'accept-a'
$instB = 'accept-b'
$homeA = Join-Path $homesDir 'a'
$homeB = Join-Path $homesDir 'b'

foreach ($pair in @(@($instA, 'A', $homeA), @($instB, 'B', $homeB))) {
  $id = $pair[0]; $label = $pair[1]; $home = $pair[2]
  New-Item -ItemType Directory -Force -Path $home | Out-Null
  $body = [ordered]@{
    schemaVersion = 1
    id = $id
    name = "accept-$label"
    version = $tag
    home = $home
    archived = $false
    createdAt = (Get-Date).ToString('o')
    lastLaunchAt = $null
    lastExitClean = $true
    notes = 'real-machine acceptance (tools/accept-real.ps1)'
    safeMode = $false
  }
  $body | ConvertTo-Json | Set-Content -Path (Join-Path $instancesDir "$id.json") -Encoding utf8
}

# ------------------------------------------------------------------ A1 launch A

$exit = Invoke-Cli @('--e2e-launch', $instA) $cliOut
$res = Get-CliJson $cliOut
$running = Get-RunningHomes $res
Write-Check 'A1 launch A -> running' ([bool]$res.ok -and $res.phase -eq 'running') ("phase={0} shell={1} host={2} {3}ms" -f $res.phase, $res.shellPid, $res.hostPid, $res.elapsedMs)
Write-Check 'A1 a process exists for A home' ($running | Where-Object { (Normalize $_) -eq (Normalize $homeA) }) ''
$afterA = if (Test-Path $pointerPath) { Get-Content $pointerPath -Raw } else { $null }
$pointerHomeA = if ($afterA) { (ConvertFrom-Json $afterA).home } else { $null }
Write-Check 'A1 pointer restored to default home' ((Normalize $pointerHomeA) -eq (Normalize $defaultHome)) ("pointer=$pointerHomeA default=$defaultHome")
$lastHomeA = if (Test-Path $lastHomePath) { (Get-Content $lastHomePath -Raw | ConvertFrom-Json).home } else { $null }
Write-Check 'A4 last-home synced with pointer' ((Normalize $lastHomeA) -eq (Normalize $pointerHomeA)) ("lastHome=$lastHomeA")

# ------------------------------------------------------------------ A2 launch B concurrently

$exit = Invoke-Cli @('--e2e-launch', $instB) $cliOut
$res = Get-CliJson $cliOut
$running = Get-RunningHomes $res
$hasA = [bool]($running | Where-Object { (Normalize $_) -eq (Normalize $homeA) })
$hasB = [bool]($running | Where-Object { (Normalize $_) -eq (Normalize $homeB) })
Write-Check 'A2 launch B -> running' ([bool]$res.ok -and $res.phase -eq 'running') ("phase={0} shell={1} host={2}" -f $res.phase, $res.shellPid, $res.hostPid)
Write-Check 'A2 both homes running at once' ($hasA -and $hasB) ("A={0} B={1}" -f $hasA, $hasB)

# ------------------------------------------------------------------ A3 duplicate home rejected

$pointerBeforeDup = if (Test-Path $pointerPath) { Get-Content $pointerPath -Raw } else { $null }
$exit = Invoke-Cli @('--e2e-launch', $instA) $cliOut
$dup = Get-CliJson $cliOut
$pointerAfterDup = if (Test-Path $pointerPath) { Get-Content $pointerPath -Raw } else { $null }
$code = if ($dup -and $dup.error) { $dup.error.code } else { '' }
Write-Check 'A3 duplicate launch rejected E1003' ($code -eq 'E1003') ("code=$code")
Write-Check 'A3 pointer untouched by the rejected launch' ($pointerBeforeDup -eq $pointerAfterDup) ''

# ------------------------------------------------------------------ A5 stop

$exit = Invoke-Cli @('--e2e-stop', $instA) $cliOut
$stopA = Get-CliJson $cliOut
Write-Check 'A5 graceful stop A' ([bool]$stopA.stopped) ("needsForce={0} pid={1}" -f $stopA.needsForce, $stopA.pid)

$exit = Invoke-Cli @('--e2e-stop', $instB, '--force') $cliOut
$stopB = Get-CliJson $cliOut
Write-Check 'A5 forced stop B' ([bool]$stopB.stopped) ("pid={0}" -f $stopB.pid)

Start-Sleep -Seconds 2
$null = Invoke-Cli @('--e2e-status') $cliOut
$status = Get-CliJson $cliOut
$still = @((Get-RunningHomes $status) | Where-Object { (Normalize $_) -in @((Normalize $homeA), (Normalize $homeB)) })
Write-Check 'A5 no leftover process for test homes' ($still.Count -eq 0) ($still -join ', ')

# ------------------------------------------------------------------ A8 appData isolation probe

# Informational: does the REAL DSH-NEXT honour a private %APPDATA%?
# Upstream resolves the location file under app.getPath('appData') (lib/main.js:3030-3031), so if
# Electron takes the APPDATA environment variable, the instance writes its own
# "DSH NEXT\home\desktop-next-last-home.json" inside our private dir. If that file appears, the
# restart cross-talk can be fixed for good (see docs-private/decision-appdata.md).
$cfgPath = Join-Path $dataDir 'app.json'
$cfgBeforeA8 = Get-Content $cfgPath -Raw
$idIso = 'accept-iso'
$homeIso = Join-Path $homesDir 'iso'
New-Item -ItemType Directory -Force -Path $homeIso | Out-Null
@{
  schemaVersion = 1; id = $idIso; name = 'accept-iso'; version = $tag; home = $homeIso
  archived = $false; createdAt = (Get-Date).ToString('o'); lastLaunchAt = $null
  lastExitClean = $true; notes = 'appdata isolation probe'; safeMode = $false
} | ConvertTo-Json | Set-Content -Path (Join-Path $instancesDir "$idIso.json") -Encoding utf8

# The config flip is wrapped in try/finally: a Ctrl-C or a failed launch in the middle of this
# probe must not leave isolateAppData=true in the user's real app.json.
try {
  $cfgObj = $cfgBeforeA8 | ConvertFrom-Json
  $cfgObj | Add-Member -NotePropertyName isolateAppData -NotePropertyValue $true -Force
  $cfgObj | ConvertTo-Json -Depth 8 | Set-Content -Path $cfgPath -Encoding utf8

  $null = Invoke-Cli @('--e2e-launch', $idIso) $cliOut
  $iso = Get-CliJson $cliOut
  Write-Check 'A8 隔离模式仍能启动' ([bool]$iso.ok -and $iso.phase -eq 'running') ("phase={0}" -f $iso.phase)
  $privateAppData = Join-Path $dataDir (Join-Path 'appdata' $idIso)
  $stamp = $null
  if (Test-Path $privateAppData) {
    $stamp = Get-ChildItem -Path $privateAppData -Recurse -Filter 'desktop-next-last-home.json' -ErrorAction SilentlyContinue |
      Select-Object -First 1
  }
  if ($stamp) {
    Write-Check 'A8 上游认私有 APPDATA（重启串台可在实例内自愈）' $true $stamp.FullName
  } else {
    Write-Host ('INFO  A8 上游没有在私有 APPDATA 下落定位文件 -- 该版本忽略 APPDATA 环境变量，' +
      '重启仍走全局指针；isolateAppData 保持关闭。') -ForegroundColor Yellow
  }
  $null = Invoke-Cli @('--e2e-stop', $idIso, '--force') $cliOut
} finally {
  Set-Content -Path $cfgPath -Value $cfgBeforeA8 -NoNewline
  Remove-Item (Join-Path $instancesDir "$idIso.json") -Force -ErrorAction SilentlyContinue
}

# ------------------------------------------------------------------ A6 cleanup

if (-not $Keep) {
  foreach ($id in $instA, $instB) {
    $p = Join-Path $instancesDir "$id.json"
    if (Test-Path $p) { Remove-Item $p -Force }
  }
  Start-Sleep -Milliseconds 300
  Remove-Item $homesDir -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
  if ($null -eq $pointerBefore) { Remove-Item $pointerPath -Force -ErrorAction SilentlyContinue }
  else { Set-Content -Path $pointerPath -Value $pointerBefore -NoNewline }
}
$pointerAfter = if (Test-Path $pointerPath) { Get-Content $pointerPath -Raw } else { $null }
Write-Check 'A6 pointer file identical to its original text' ($pointerAfter -eq $pointerBefore) ''
Write-Check 'A6 test instances removed' (-not (Test-Path (Join-Path $instancesDir "$instA.json"))) ''

# ------------------------------------------------------------------ summary

Write-Host ''
Write-Host ("{0} passed, {1} failed" -f $script:PASS, $script:FAIL)
if ($script:FAIL -gt 0) {
  Write-Host ('failed: ' + ($failures -join ', ')) -ForegroundColor Red
  exit 1
}

Write-Host ''
Write-Host 'Manual checks that no script can do (do them once on this machine):'
Write-Host '  1) Launch two instances from the GUI; two DSH-NEXT windows appear, each on its own home.'
Write-Host '  2) Switch to the second instance: no "data directory differs from last time" dialog.'
Write-Host '  3) Restart menu: normal / enter safe mode / leave safe mode; safe-mode badge shows.'
Write-Host '  4) Stop: the window closes itself within 15s (graceful), not by taskkill /F.'
exit 0
