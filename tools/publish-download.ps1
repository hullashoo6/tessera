# Puts a tested installer on the website: copies it to website\public\downloads
# and records its version, exact size and SHA-256 in website\src\data\releases.json
# (the download section shows only entries with all three). Other platforms'
# entries are kept. Deploy the website afterwards (docs/WEBSITE.md).
#   tools\publish-download.ps1 -Platform windows            (target\release\bundle\nsis\Tessera_<v>_x64-setup.exe)
#   tools\publish-download.ps1 -Platform mac -File <dmg>    (from the release workflow, after its Mac tests passed)
# Run only for an installer that passed its tests (tools\release.ps1 for
# Windows; the release workflow's install-and-launch tests for Mac).
param(
  [Parameter(Mandatory)][ValidateSet('windows', 'mac')][string]$Platform,
  [string]$File
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$conf = [IO.File]::ReadAllText((Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$version = $conf.version
if ($Platform -eq 'windows') {
  $name = "Tessera_$($version)_x64-setup.exe"
  if (-not $File) { $File = Join-Path $root "target\release\bundle\nsis\$name" }
  $id = 'windows-x64'; $os = 'Windows 11, 64-bit'; $pattern = 'Tessera_*_x64-setup.exe'
} else {
  $name = "Tessera_$($version)_universal.dmg"
  $id = 'mac-universal'; $os = 'macOS 12 or later, Apple silicon and Intel'; $pattern = 'Tessera_*_universal.dmg'
}
if (-not $File -or -not (Test-Path $File)) { throw "Installer not found: $File" }
if ((Split-Path $File -Leaf) -notlike "*$version*") { throw "$File is not version $version" }
$downloads = Join-Path $root 'website\public\downloads'
New-Item -ItemType Directory -Force $downloads | Out-Null
# Only the current installer of this platform is kept on the site.
Get-ChildItem $downloads -Filter $pattern | Where-Object { $_.Name -ne $name } | Remove-Item
Copy-Item $File (Join-Path $downloads $name) -Force
$item = Get-Item (Join-Path $downloads $name)
$sha = (Get-FileHash $item.FullName -Algorithm SHA256).Hash.ToLower()
$entry = [ordered]@{
  platform = $id; version = $version; installer = "for $Platform"; file = $name
  url = "https://tessera-96w.pages.dev/downloads/$name"; bytes = $item.Length; sha256 = $sha
  minimumOs = $os; notesUrl = "/changelog#$version"
} | ConvertTo-Json -Compress
# Merged with node (never Windows PowerShell 5.1's JSON for files we keep).
$manifest = Join-Path $root 'website\src\data\releases.json'
node -e "const fs=require('fs');const f=process.argv[1];const e=JSON.parse(process.argv[2]);const j=JSON.parse(fs.readFileSync(f,'utf8'));j.comment='Written by tools/publish-download.ps1 from installers that passed their tests. Never edit by hand to add a release that has not passed installed-build testing.';j.releases=(j.releases||[]).filter(r=>r.platform!==e.platform).concat([e]).sort((a,b)=>a.platform.localeCompare(b.platform));fs.writeFileSync(f,JSON.stringify(j,null,2)+'\n')" $manifest $entry
if ($LASTEXITCODE -ne 0) { throw 'could not update releases.json' }
"published $name ($($item.Length) bytes, sha256 $sha) for $id"
