# Builds an unsigned MSIX package for Microsoft Store submission (the Store
# signs it). Identity values come from Partner Center > Product identity.
# Prepared but untested until those values exist. See docs/STORE.md.
param(
  [Parameter(Mandatory = $true)][string]$Name,
  [Parameter(Mandatory = $true)][string]$Publisher,
  [Parameter(Mandatory = $true)][string]$PublisherDisplayName,
  # Store versions have four parts and the last must be 0; default: the app's version + ".0".
  [string]$Version = ""
)
$root = Split-Path $PSScriptRoot -Parent
if (-not $Version) {
  $conf = [IO.File]::ReadAllText((Join-Path $root 'apps\desktop\src-tauri\tauri.conf.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
  $Version = "$($conf.version).0"
}
$exe = Join-Path $root 'target\release\tessera-desktop.exe'
if (-not (Test-Path $exe)) { throw "Build the app first: npx tauri build (in apps\desktop)" }
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $sdk) { throw "makeappx.exe not found (Windows SDK, installed with the Build Tools)" }

$layout = Join-Path $root 'target\msix\layout'
Remove-Item $layout -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $layout 'Assets') | Out-Null
Copy-Item $exe (Join-Path $layout 'tessera-desktop.exe')
# Licence credits ship next to the program, as in the .exe installer.
Copy-Item (Join-Path $root 'THIRD_PARTY_NOTICES.md') (Join-Path $layout 'THIRD_PARTY_NOTICES.md')
$icons = Join-Path $root 'apps\desktop\src-tauri\icons'
Copy-Item (Join-Path $icons 'StoreLogo.png') (Join-Path $layout 'Assets\StoreLogo.png')
Copy-Item (Join-Path $icons 'Square150x150Logo.png') (Join-Path $layout 'Assets\Square150x150Logo.png')
Copy-Item (Join-Path $icons 'Square44x44Logo.png') (Join-Path $layout 'Assets\Square44x44Logo.png')

$manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="$Name" Publisher="$Publisher" Version="$Version" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Tessera</DisplayName>
    <PublisherDisplayName>$PublisherDisplayName</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.22000.0" MaxVersionTested="10.0.26300.0" />
  </Dependencies>
  <Resources><Resource Language="en-us" /></Resources>
  <Applications>
    <Application Id="Tessera" Executable="tessera-desktop.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Tessera" Description="See what fills your storage."
        BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"@
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest, (New-Object Text.UTF8Encoding($false)))
$out = Join-Path $root "target\msix\Tessera_$Version`_x64.msix"
& $sdk.FullName pack /d $layout /p $out /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
"wrote $out (unsigned; the Microsoft Store signs it on submission)"
