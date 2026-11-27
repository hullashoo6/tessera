# Removes from the Recycle Bin only the items that came from a test fixture
# folder (inside this repository's .fixtures folder), so cleanup tests on a
# real computer leave nothing behind. Every other Recycle Bin item is left
# untouched. Also takes those items off Tessera's Restore list.
#   powershell -File tools\bin-purge-fixture.ps1 -Under C:\...\tessera\.fixtures\cleanup-trial
param(
    [Parameter(Mandatory)] [string] $Under,
    # For testing this script on a copy; defaults to Tessera's real list.
    [string] $History = (Join-Path $env:LOCALAPPDATA 'app.tessera.desktop\recently-moved.json')
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$fixtures = [IO.Path]::GetFullPath((Join-Path $repo '.fixtures')).TrimEnd('\') + '\'
$under = [IO.Path]::GetFullPath($Under).TrimEnd('\') + '\'
if (-not $under.StartsWith($fixtures, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing: $Under is not inside $fixtures"
}
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$bin = "$($env:SystemDrive)\`$Recycle.Bin\$sid"
$removed = 0
foreach ($info in Get-ChildItem -LiteralPath $bin -Force -Filter '$I*' -ErrorAction SilentlyContinue) {
    $b = [IO.File]::ReadAllBytes($info.FullName)
    # $I format 2 (Windows 10+): version 8 bytes, size 8, deletion time 8,
    # name length 4 (characters, with the final NUL), then the UTF-16 name.
    if ($b.Length -lt 28 -or [BitConverter]::ToInt64($b, 0) -ne 2) { continue }
    $chars = [BitConverter]::ToInt32($b, 24)
    $original = [Text.Encoding]::Unicode.GetString($b, 28, [Math]::Max(0, ($chars - 1) * 2))
    if (-not ($original + '\').StartsWith($under, [StringComparison]::OrdinalIgnoreCase)) { continue }
    $r = Join-Path $bin ('$R' + $info.Name.Substring(2))
    if (Test-Path -LiteralPath $r) { Remove-Item -LiteralPath $r -Recurse -Force }
    Remove-Item -LiteralPath $info.FullName -Force
    $removed++
}
"Removed $removed fixture item(s) from the Recycle Bin."

# Tessera's Restore list: drop only the fixture's entries. The list holds the
# owner's own entries too, so: back it up first, edit it with Node (Windows
# PowerShell 5.1's ConvertFrom-Json turns a JSON list into one object, which
# once wiped the whole list here), and never drop anything outside $Under.
$history = $History
if (Test-Path $history) {
    $backup = Join-Path $repo ("target\recently-moved.{0:yyyyMMdd-HHmmss}.bak.json" -f (Get-Date))
    New-Item -ItemType Directory -Force (Split-Path $backup) | Out-Null
    Copy-Item $history $backup
    $js = @'
const fs = require('fs')
const [file, under] = process.argv.slice(2)
const items = JSON.parse(fs.readFileSync(file, 'utf8'))
if (!Array.isArray(items)) throw new Error('Restore list is not a list; left unchanged')
const inside = (p) => (String(p) + '\\').toLowerCase().startsWith(under.toLowerCase())
const keep = items.filter((m) => !inside(m.original))
if (keep.some((m) => inside(m.original)) || keep.length + items.filter((m) => inside(m.original)).length !== items.length)
  throw new Error('count mismatch; left unchanged')
if (keep.length !== items.length) fs.writeFileSync(file, JSON.stringify(keep, null, 2))
console.log(`Took ${items.length - keep.length} fixture entries off the Restore list; ${keep.length} kept.`)
'@
    $tmp = Join-Path $env:TEMP 'tessera-purge-history.js'
    Set-Content -Path $tmp -Value $js -Encoding ASCII
    node $tmp $history $under
    if ($LASTEXITCODE -ne 0) { Copy-Item $backup $history -Force; throw 'Restore list edit failed; the backup was put back.' }
    Remove-Item $tmp
    "Backup of the list before the edit: $backup"
}
