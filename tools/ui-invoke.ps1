# Presses a named button in the Tessera window through Windows UI Automation
# (the accessibility API), without moving the mouse. Used for UI checks.
# Usage: powershell -File tools\ui-invoke.ps1 -Name "Largest files"
param([Parameter(Mandatory = $true)][string]$Name, [string]$Process = "tessera-desktop")
. (Join-Path $PSScriptRoot 'ui-window.ps1')
$root = Get-TesseraWindow $Process
$cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::NameProperty), $Name
$el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
if (-not $el) {
  # Fall back to the first element whose name starts with $Name (e.g. a
  # button whose name also includes a count badge).
  $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($e in $all) { if ($e.Current.Name -like "$Name*") { $el = $e; break } }
}
if (-not $el) { throw "No element named '$Name'" }
$pattern = $null
if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
  $pattern.Invoke()
  "invoked '$Name'"
} elseif ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
  $pattern.Toggle()
  "toggled '$Name'"
} else {
  throw "'$Name' cannot be invoked"
}
