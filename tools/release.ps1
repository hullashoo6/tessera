# Builds and checks a Windows release candidate on this laptop.
# Nothing is signed, uploaded or published.
#
#   powershell -File tools\release.ps1               # checks, build, install, smoke test, manifest
#   powershell -File tools\release.ps1 -SkipChecks   # when tools\check.ps1 has just passed
#
# Steps:
#  1. tools\check.ps1 -Quick (format, lint, all tests, macOS compile check, website build)
#  2. npx tauri build --bundles nsis
#  3. silent install over the current installation, then an automated smoke test of the
#     installed copy: right-click menu entries present; launched on a disposable folder,
#     the window appears and answers within 1 s throughout (Windows calls a window
#     "Not responding" after 5 s); the local log records the start with this version;
#     closing the window quits (or hides it, if background mode is on).
#  4. tools\release-manifest.ps1 (file, size, SHA-256, signed = false, verified = false)
#
# Still manual afterwards (see TESTING.md): cleanup with disposable files and Put back,
# licence check, Explorer's right-click menu itself. The Mac DMG is built only by the
# release workflow on GitHub (release.yml).
param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:NODE_ENV = 'development'
$conf = [IO.File]::ReadAllText((Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$version = $conf.version
function Fail([string]$why) { Write-Host "RELEASE CHECK FAILED: $why" -ForegroundColor Red; exit 1 }
function Ok([string]$what) { Write-Host "  ok  $what" -ForegroundColor Green }

if (-not $SkipChecks) {
  Write-Host "== 1. Checks" -ForegroundColor Cyan
  powershell -NoProfile -File (Join-Path $PSScriptRoot 'check.ps1') -Quick
  if ($LASTEXITCODE -ne 0) { Fail 'tools\check.ps1 found problems' }
}

Write-Host "== 2. Build the Windows installer ($version)" -ForegroundColor Cyan
Push-Location apps\desktop
# A release must be built in production mode. NODE_ENV=development (set
# above for the test tools) makes Vite bundle React's development version,
# which is slower and deliberately runs every screen's effects twice; all
# local installers from 27 Nov ~03:50 to ~20:30 IST shipped it.
$env:NODE_ENV = 'production'
# Through cmd: Windows PowerShell 5.1 turns a redirected program's progress
# messages on stderr into errors, which would stop this script.
cmd /c "npx tauri build --bundles nsis > ""$root\target\release-build.log"" 2>&1"
$built = $LASTEXITCODE
$env:NODE_ENV = 'development'
Pop-Location
if ($built -ne 0) { Fail 'the build failed (see target\release-build.log)' }
powershell -NoProfile -File (Join-Path $PSScriptRoot 'check-production-bundle.ps1')
if ($LASTEXITCODE -ne 0) { Fail 'the interface was built in development mode' }
Ok 'interface built in production mode'
$setup = Get-ChildItem 'target\release\bundle\nsis' -Filter "Tessera_$($version)_x64-setup.exe" | Select-Object -First 1
if (-not $setup) { Fail 'installer not found' }
Ok ("{0} ({1:N2} MB)" -f $setup.Name, ($setup.Length / 1MB))

Write-Host "== 3. Install and smoke test" -ForegroundColor Cyan
Get-Process tessera-desktop -ErrorAction SilentlyContinue | Stop-Process
Start-Sleep 1
$p = Start-Process $setup.FullName -ArgumentList '/S' -PassThru -Wait
if ($p.ExitCode -ne 0) { Fail "installer exit code $($p.ExitCode)" }
$exe = Join-Path $env:LOCALAPPDATA 'Tessera\tessera-desktop.exe'
if (-not (Test-Path $exe)) { Fail 'installed program not found' }
Ok 'silent install'
foreach ($k in 'Directory', 'Directory\Background', 'Drive') {
  if (-not (Test-Path "HKCU:\Software\Classes\$k\shell\Tessera\command")) { Fail "right-click entry missing for $k" }
}
Ok 'right-click "Scan with Tessera" entries'

$fx = Join-Path $root '.fixtures\release-smoke'
Remove-Item $fx -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $fx 'web\node_modules\dep') | Out-Null
Set-Content (Join-Path $fx '.tessera-fixture') 'disposable'
Set-Content (Join-Path $fx 'web\package.json') '{}'
[IO.File]::WriteAllBytes((Join-Path $fx 'web\node_modules\dep\index.js'), (New-Object byte[] (2MB)))
$log = Join-Path $env:LOCALAPPDATA 'app.tessera.desktop\logs\tessera.log'
$logBefore = if (Test-Path $log) { (Get-Item $log).Length } else { 0 }
# The smoke test's scan must not end up in the owner's storage History
# (28 Nov 2026: 31 test scans had collected there). Copied byte for byte
# and put back after the test; never edited here.
$hist = Join-Path $env:LOCALAPPDATA 'app.tessera.desktop\storage-history.json'
$histBackup = Join-Path $root 'target\storage-history.before-smoke.json'
$hadHist = Test-Path $hist
if ($hadHist) { Copy-Item $hist $histBackup -Force }

Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public static class ReleaseProbe {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint f, uint t, out IntPtr r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  // Tessera's own window (class "Tauri Window"). The process also owns small
  // helper windows (single instance, event loop, tray) that must not count.
  public static IntPtr MainWindow(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      var c = new StringBuilder(64); GetClassName(h, c, 64);
      if (p == pid && c.ToString() == "Tauri Window") { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@
$psi = New-Object Diagnostics.ProcessStartInfo $exe, "`"$fx`""
$psi.UseShellExecute = $true
$app = [Diagnostics.Process]::Start($psi)
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 30; $i++) {
  Start-Sleep 1
  $h = [ReleaseProbe]::MainWindow([uint32]$app.Id)
  if ($h -ne [IntPtr]::Zero -and [ReleaseProbe]::IsWindowVisible($h)) { break }
}
if ($h -eq [IntPtr]::Zero -or -not [ReleaseProbe]::IsWindowVisible($h)) { Fail 'the Tessera window did not appear within 30 s' }
Ok 'window appeared'
$win = Get-Process -Id $app.Id
$worst = 0.0
$r = [IntPtr]::Zero
$sw = [Diagnostics.Stopwatch]::StartNew()
while ($sw.Elapsed.TotalSeconds -lt 10) {
  $t = [Diagnostics.Stopwatch]::StartNew()
  [void][ReleaseProbe]::SendMessageTimeout($h, 0, [IntPtr]::Zero, [IntPtr]::Zero, 2, 5000, [ref]$r)
  $worst = [Math]::Max($worst, $t.Elapsed.TotalMilliseconds)
  Start-Sleep -Milliseconds 100
}
if ($worst -gt 1000) { Fail ("the window took {0:N0} ms to respond" -f $worst) }
Ok ("window responsive for 10 s (slowest answer {0:N0} ms)" -f $worst)
if (-not (Test-Path $log)) { Fail 'no diagnostics log written' }
$fresh = [IO.File]::ReadAllText($log).Substring([int]$logBefore)
if ($fresh -notmatch "start: Tessera $([regex]::Escape($version))") { Fail 'the log has no start line for this version' }
if ($fresh -match 'crash:') { Fail 'the log recorded a crash' }
Ok 'log records the start, no crash'
[void][ReleaseProbe]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) # WM_CLOSE, like the close button
Start-Sleep 4
if (Get-Process -Id $app.Id -ErrorAction SilentlyContinue) {
  if ([ReleaseProbe]::IsWindowVisible($h)) { Fail 'closing the window did nothing' }
  Ok 'closing hid the window (background mode is on); stopping it'
  Stop-Process -Id $app.Id
} else {
  Ok 'closing the window quit'
}
if (Test-Path (Join-Path $fx '.tessera-fixture')) { Remove-Item $fx -Recurse -Force }
if ($hadHist) { Copy-Item $histBackup $hist -Force } elseif (Test-Path $hist) { Remove-Item $hist }
Ok 'storage History left as it was'

Write-Host "== 4. Manifest" -ForegroundColor Cyan
powershell -NoProfile -File (Join-Path $PSScriptRoot 'release-manifest.ps1') -Notes "Release candidate built locally $(Get-Date -Format 'yyyy-MM-dd'); automated smoke test passed"
Write-Host "`nRelease candidate $version is built, installed on this computer and smoke-tested." -ForegroundColor Green
Write-Host "Nothing was signed, uploaded or published. Remaining manual checks: TESTING.md."
