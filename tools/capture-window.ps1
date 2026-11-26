# Captures only the Tessera window (not the rest of the screen) to a PNG.
# Usage: powershell -File tools\capture-window.ps1 -Out target\shot.png
param([string]$Out = "target\shot.png", [string]$Process = "tessera-desktop", [int]$MaxWidth = 1400)

Add-Type -ReferencedAssemblies System.Drawing @"
using System;
using System.Runtime.InteropServices;
public static class TesseraCapture {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int L, T, R, B; }
}
"@
[void][TesseraCapture]::SetProcessDPIAware()
. (Join-Path $PSScriptRoot 'ui-window.ps1'); $win = Get-TesseraWindow $Process; $p = [pscustomobject]@{ MainWindowHandle = [IntPtr]$win.Current.NativeWindowHandle }
if (-not $p) { throw "No visible $Process window" }
$r = New-Object TesseraCapture+RECT
[void][TesseraCapture]::GetWindowRect($p.MainWindowHandle, [ref]$r)
$w = $r.R - $r.L; $h = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[void][TesseraCapture]::PrintWindow($p.MainWindowHandle, $hdc, 2)  # PW_RENDERFULLCONTENT
$g.ReleaseHdc($hdc); $g.Dispose()
if ($w -gt $MaxWidth) {
  $nh = [int]($h * $MaxWidth / $w)
  $small = New-Object System.Drawing.Bitmap $bmp, $MaxWidth, $nh
  $bmp.Dispose(); $bmp = $small
}
$bmp.Save((Join-Path (Get-Location) $Out), [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"saved $Out ($w x $h)"
