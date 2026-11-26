# Presses a named button in the Tessera window through Windows UI Automation
# (the accessibility API), without moving the mouse. Used for UI checks.
# Usage: powershell -File tools\ui-invoke.ps1 -Name "Largest files"
param([Parameter(Mandatory = $true)][string]$Name, [string]$Process = "tessera-desktop")
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$p = Get-Process $Process -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
$root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
$cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::NameProperty), $Name
$el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
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
