# Fails (exit 1) if the built interface (apps\desktop\dist) contains React's
# development version, which happens when NODE_ENV=development is set during
# `vite build` / `tauri build`. Release builds must never ship it: it is
# slower and runs every screen's effects twice on purpose.
$root = Split-Path $PSScriptRoot -Parent
$files = Get-ChildItem (Join-Path $root 'apps\desktop\dist\assets') -Filter '*.js' -ErrorAction SilentlyContinue
if (-not $files) { Write-Host 'no built interface found (apps\desktop\dist)'; exit 1 }
foreach ($f in $files) {
  $text = [IO.File]::ReadAllText($f.FullName)
  if ($text.Contains('jsxDEV') -or $text.Contains('react-stack-bottom-frame')) {
    Write-Host "$($f.Name) contains React's development build"
    exit 1
  }
}
Write-Host "production bundle ok ($($files.Count) file(s))"
exit 0
