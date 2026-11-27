# Puts the Windows installer built by tools\release.ps1 on the website:
# copies it to website\public\downloads and records its version, exact size
# and SHA-256 in website\src\data\releases.json (the download section shows
# only entries with all three). Run only for an installer that passed
# release.ps1's installed-build smoke test. Deploy the website afterwards
# (docs/WEBSITE.md). Publishing needs the owner's OK.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$conf = [IO.File]::ReadAllText((Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$version = $conf.version
$name = "Tessera_$($version)_x64-setup.exe"
$src = Join-Path $root "target\release\bundle\nsis\$name"
if (-not (Test-Path $src)) { throw "Installer not found: $src (run tools\release.ps1 first)" }
$downloads = Join-Path $root 'website\public\downloads'
New-Item -ItemType Directory -Force $downloads | Out-Null
# Only the current installer is kept on the site.
Get-ChildItem $downloads -Filter 'Tessera_*_x64-setup.exe' | Where-Object { $_.Name -ne $name } | Remove-Item
Copy-Item $src (Join-Path $downloads $name) -Force
$file = Get-Item (Join-Path $downloads $name)
$sha = (Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLower()
$site = 'https://tessera-96w.pages.dev'
$manifest = [ordered]@{
  comment = 'Written by tools/publish-windows-download.ps1 from an installer that passed tools/release.ps1. Never edit by hand to add a release that has not passed installed-build testing.'
  releases = @([ordered]@{
    platform = 'windows-x64'
    version = $version
    installer = 'for Windows'
    file = $name
    url = "$site/downloads/$name"
    bytes = $file.Length
    sha256 = $sha
    minimumOs = 'Windows 11, 64-bit'
    notesUrl = "/changelog#$version"
  })
}
$json = $manifest | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText((Join-Path $root 'website\src\data\releases.json'), $json + "`n", (New-Object Text.UTF8Encoding($false)))
"published $name ($($file.Length) bytes, sha256 $sha) in website\public\downloads and releases.json"
