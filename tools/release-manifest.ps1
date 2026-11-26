# Writes a machine-readable manifest for the Windows installer(s) in
# target\release\bundle. Nothing is uploaded or published.
# Usage: powershell -File tools\release-manifest.ps1 [-Verified] [-Notes "..."]
#   -Verified  only after install / launch / scan / cleanup / uninstall
#              testing of this exact file has passed (see TESTING.md).
param([switch]$Verified, [string]$Notes = "")
$root = Split-Path $PSScriptRoot -Parent
$conf = Get-Content (Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$dir = Join-Path $root 'target\release\bundle\nsis'
$items = @()
foreach ($f in Get-ChildItem $dir -Filter '*.exe' -ErrorAction SilentlyContinue) {
  $sig = Get-AuthenticodeSignature $f.FullName
  $items += [ordered]@{
    platform  = 'windows-x64'
    version   = $conf.version
    installer = 'NSIS setup (.exe), per-user'
    file      = $f.Name
    bytes     = $f.Length
    sha256    = (Get-FileHash $f.FullName -Algorithm SHA256).Hash.ToLower()
    signed    = ($sig.Status -eq 'Valid')
    verified  = [bool]$Verified
    url       = $null   # set only when hosted and approved for publication
    minimumOs = 'Windows 11, 64-bit'
    notes     = $Notes
    builtAt   = (Get-Date).ToUniversalTime().ToString('o')
  }
}
if (-not $items) { throw "No installer found in $dir" }
$out = Join-Path $root 'target\release\bundle\manifest.json'
[IO.File]::WriteAllText($out, (ConvertTo-Json @{ releases = $items } -Depth 4), (New-Object Text.UTF8Encoding($false)))
"wrote $out"
$items | ForEach-Object { "{0}  {1:N1} MB  sha256 {2}  signed={3}  verified={4}" -f $_.file, ($_.bytes / 1MB), $_.sha256, $_.signed, $_.verified }
