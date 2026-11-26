# Types text into the first editable field named $Name in the Tessera window
# using UI Automation's ValuePattern (no keyboard or mouse involved).
param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][string]$Text, [string]$Process = "tessera-desktop")
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$p = Get-Process $Process -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
$root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
$cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::NameProperty), $Name
$el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
if (-not $el) { throw "No element named '$Name'" }
$vp = $el.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
$vp.SetValue($Text)
"typed into '$Name'"
