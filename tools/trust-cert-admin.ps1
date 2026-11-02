# trust-cert.ps1 —— 把自签名代码签名证书装进「本机」受信任存储（需管理员）
#
# 作用：让 SmartScreen 与 UAC 的"发布者"字段认可本机构建的 exe。
# 只对**本机**有效；别人下载你的 exe 仍然会被拦（他们没有这张证书）。
#
# 用法（必须以管理员身份运行）：
#   pwsh -File tools\trust-cert-admin.ps1
#   或：右键 "以管理员身份运行 PowerShell" → 执行同一行

$ErrorActionPreference = 'Stop'

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Host "需要管理员权限。请右键「以管理员身份运行」后重试。" -ForegroundColor Yellow
  exit 1
}

$cert = Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.Subject -like "*DSH-NEXT Multi-Launcher*" -and $_.HasPrivateKey } |
  Sort-Object NotAfter -Descending |
  Select-Object -First 1

if (-not $cert) {
  Write-Host "找不到自签名证书。先运行：pnpm sign" -ForegroundColor Yellow
  exit 1
}
Write-Host "证书: $($cert.Thumbprint)  到期: $($cert.NotAfter)"

# 1) 导入到本机 Root（链验证）与 TrustedPublisher（发布者信任）
foreach ($storeName in @('Root', 'TrustedPublisher')) {
  $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, 'LocalMachine')
  $store.Open('ReadWrite')
  $exists = $store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
  if ($exists) {
    Write-Host "LocalMachine\$storeName 已存在，跳过"
  } else {
    $store.Add($cert)
    Write-Host "已导入 LocalMachine\$storeName"
  }
  $store.Close()
}

# 2) 重新验证 exe 签名
$exe = Join-Path (Split-Path -Parent $PSScriptRoot) 'DSH-NEXT-Multi.exe'
if (Test-Path $exe) {
  $sig = Get-AuthenticodeSignature $exe
  Write-Host "签名状态: $($sig.Status)"
  Write-Host "发布者: $($sig.SignerCertificate.Subject)"
}

# 3) 提示 SmartScreen 仍可能对"新发布者"提示一次
Write-Host ""
Write-Host "完成。注意：" -ForegroundColor Green
Write-Host " - 本机：SmartScreen 通常不再红屏，UAC 会显示 'DSH-NEXT Multi-Launcher'"
Write-Host " - 其它机器：自签名无效，仍会提示（这是自签名的固有限制）"
