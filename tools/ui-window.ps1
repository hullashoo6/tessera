# Shared helper: finds the real Tessera window (largest top-level window of
# the process), since Process.MainWindowHandle can point at a helper window.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
function Get-TesseraWindow([string]$Process = "tessera-desktop") {
  $ids = @(Get-Process $Process -ErrorAction Stop | ForEach-Object { $_.Id })
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Window)
  $best = $null; $area = 0
  foreach ($w in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)) {
    if ($ids -contains $w.Current.ProcessId) {
      $r = $w.Current.BoundingRectangle
      if ($r.Width * $r.Height -gt $area) { $best = $w; $area = $r.Width * $r.Height }
    }
  }
  if (-not $best) { throw "No visible $Process window" }
  return $best
}
