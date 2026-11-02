# capture-window.ps1 —— 按窗口标题抓图（PrintWindow 优先，失败退回屏幕拷贝）。
# 用法：pwsh -File tools/capture-window.ps1 -TitleLike "DSH-NEXT 应用多开启动器" -Out target/ui.png

param(
  [string]$TitleLike = "DSH-NEXT",
  [string]$Out = "target/capture.png"
)

Add-Type -AssemblyName System.Drawing

$sig = @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public class WinCap
{
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    // 让本进程按物理像素工作：否则在 150% 缩放下 GetWindowRect 会返回“虚拟化”后的尺寸，
    // 截出来的图只有窗口左上 2/3（真实 1680×1140 被报成 1135×797）。
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public static IntPtr Find(string needle)
    {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) =>
        {
            if (!IsWindowVisible(h)) return true;
            var sb = new StringBuilder(512);
            GetWindowText(h, sb, sb.Capacity);
            var text = sb.ToString();
            if (!string.IsNullOrEmpty(text) && text.IndexOf(needle, StringComparison.OrdinalIgnoreCase) >= 0)
            {
                found = h;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

Add-Type -TypeDefinition $sig

$hwnd = [WinCap]::Find($TitleLike)
if ($hwnd -eq [IntPtr]::Zero) {
  Write-Error "找不到标题包含「$TitleLike」的可见窗口"
  exit 2
}

[WinCap]::ShowWindow($hwnd, 9) | Out-Null   # SW_RESTORE
[WinCap]::SetForegroundWindow($hwnd) | Out-Null
# PER_MONITOR_AWARE_V2 = -4：必须在任何 GetWindowRect / 位图创建之前调用
[WinCap]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
Start-Sleep -Milliseconds 800

$rect = New-Object WinCap+RECT
[WinCap]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) {
  Write-Error "窗口尺寸异常：$width x $height"
  exit 3
}

$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

$bmp = New-Object System.Drawing.Bitmap ($width), ($height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [WinCap]::PrintWindow($hwnd, $hdc, 2)   # PW_RENDERFULLCONTENT
$g.ReleaseHdc($hdc)
$g.Dispose()

if (-not $ok -or $bmp.GetPixel([int]($width / 2), [int]($height / 2)).ToArgb() -eq -16777216) {
  # 退回屏幕拷贝（窗口已置前）
  $bmp.Dispose()
  $bmp = New-Object System.Drawing.Bitmap ($width), ($height)
  $g2 = [System.Drawing.Graphics]::FromImage($bmp)
  $g2.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size ($width), ($height)))
  $g2.Dispose()
}

$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output (ConvertTo-Json @{ ok = $true; out = $Out; width = $width; height = $height; hwnd = $hwnd.ToInt64() })
