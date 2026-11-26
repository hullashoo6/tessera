# Renders the macOS installer (DMG) window background with install steps.
# Output: apps\desktop\src-tauri\dmg\background.png (660 x 420)
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
$outDir = Join-Path $root 'apps\desktop\src-tauri\dmg'
New-Item -ItemType Directory -Force $outDir | Out-Null
$w = 660; $h = 420
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'; $g.TextRenderingHint = 'AntiAliasGridFit'
$g.Clear([System.Drawing.Color]::FromArgb(246, 247, 248))
$ink = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(28, 33, 39))
$muted = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(78, 89, 100))
$accent = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(62, 106, 158)), 3
$title = New-Object System.Drawing.Font 'Segoe UI Semibold', 17
$body = New-Object System.Drawing.Font 'Segoe UI', 10.5
$small = New-Object System.Drawing.Font 'Segoe UI', 9

$g.DrawString('Drag Tessera to Applications', $title, $ink, 30, 24)
# Arrow between the app icon (x≈180) and the Applications folder (x≈480).
$g.DrawLine($accent, 262, 160, 398, 160)
$g.DrawLine($accent, 398, 160, 386, 150)
$g.DrawLine($accent, 398, 160, 386, 170)

$steps = "The first time you open it, macOS may say it can't check Tessera.`n" +
         "1.  Open System Settings, then Privacy & Security.`n" +
         "2.  Scroll down to the message about Tessera and click Open Anyway.`n" +
         "3.  Confirm with Open. You only need to do this once."
$g.DrawString($steps, $body, $ink, (New-Object System.Drawing.RectangleF 30, 262, 600, 100))
$why = "Why: Tessera isn't notarised by Apple yet. Notarisation costs USD 99 a year, which we're not paying yet. " +
       "This allows only Tessera and doesn't change any other security setting."
$g.DrawString($why, $small, $muted, (New-Object System.Drawing.RectangleF 30, 358, 600, 50))

$out = Join-Path $outDir 'background.png'
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
"wrote $out"
