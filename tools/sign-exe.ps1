# sign-exe.ps1 —— 给启动器 exe 做本机自签名（消除「未知发布者」警告）。
#
# 原理：自签名代码签名证书 + 加入当前用户的「受信任的根证书颁发机构」与「受信任的发布者」。
# 注意：
#   1) 这是**本机**信任，换一台机器需要重新执行一次（证书也要重新导入）；
#   2) 自签名不能消除 SmartScreen 的「Windows 已保护你的电脑」红屏（那需要 EV 证书与信誉积累），
#      首次运行仍需点「更多信息 → 仍要运行」；
#   3) 每次重建 exe（哈希变化）都要重新签名，否则签名失效。
#
# 用法：
#   pwsh -File tools\sign-exe.ps1
#   pwsh -File tools\sign-exe.ps1 -Exe "src-tauri\target\release\DSH-NEXT-Multi.exe"

param(
  [string]$Exe = "DSH-NEXT-Multi.exe",
  [string]$Subject = "CN=DSH-NEXT Multi-Launcher, O=Local Build"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Exe)) { throw "找不到 exe：$Exe" }
$exePath = (Resolve-Path $Exe).Path
Write-Host "target: $exePath"

# 1) 复用或创建证书
$cert = Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.Subject -eq $Subject -and $_.HasPrivateKey } |
  Sort-Object NotAfter -Descending |
  Select-Object -First 1

if (-not $cert) {
  Write-Host "creating self-signed code signing certificate..."
  $cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -FriendlyName "DSH-NEXT Multi-Launcher (self-signed)" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyUsage DigitalSignature `
    -NotAfter (Get-Date).AddYears(5)
}
Write-Host "certificate: $($cert.Thumbprint)  expires: $($cert.NotAfter)"

# 2) 确保受信任（根 + 发布者）
$cer = Join-Path $env:TEMP "dsh-multi-launcher-sign.cer"
Export-Certificate -Cert $cert -FilePath $cer -Force | Out-Null

foreach ($storeName in @("Root", "TrustedPublisher")) {
  $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, "CurrentUser")
  $store.Open("ReadWrite")
  $already = $store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
  if (-not $already) {
    $store.Add($cert)
    Write-Host "added to CurrentUser\$storeName"
  } else {
    Write-Host "already trusted in CurrentUser\$storeName"
  }
  $store.Close()
}

# 3) 签名（带时间戳；离线时退回无时间戳）
Write-Host "signing..."
$sig = Set-AuthenticodeSignature -FilePath $exePath -Certificate $cert -TimestampServer "http://timestamp.digicert.com"
if ($sig.Status -ne "Valid") {
  Write-Host "timestamp failed ($($sig.StatusMessage)), retry without timestamp..."
  $sig = Set-AuthenticodeSignature -FilePath $exePath -Certificate $cert
}

# 4) 复核
$check = Get-AuthenticodeSignature $exePath
$result = [pscustomobject]@{
  exe = $exePath
  status = $check.Status
  signer = $check.SignerCertificate.Subject
  thumbprint = $cert.Thumbprint
}
$result | Format-List

if ($check.Status -ne "Valid") {
  Write-Warning "签名状态不是 Valid：$($check.StatusMessage)"
  exit 1
}
Write-Host "OK: exe signed and trusted on this machine."
