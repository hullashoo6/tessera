# Runs on this Windows laptop everything the CI workflow runs on Windows,
# plus a macOS compile check of the crates that need no Apple tools. Use it
# before pushing, and whenever GitHub Actions is unavailable (for example
# when the account's free minutes are used up).
#   powershell -File tools\check.ps1            # everything
#   powershell -File tools\check.ps1 -Quick     # skip the release-build gates
param([switch]$Quick)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:NODE_ENV = 'development'
$failed = @()
function Step([string]$name, [scriptblock]$run) {
  Write-Host "== $name" -ForegroundColor Cyan
  $global:LASTEXITCODE = 0
  & $run
  if ($LASTEXITCODE -ne 0) { $script:failed += $name; Write-Host "FAILED: $name" -ForegroundColor Red }
}
Step 'Interface: lint, tests, build' { Push-Location apps\desktop; npx oxlint src; if ($LASTEXITCODE -eq 0) { npm test }; if ($LASTEXITCODE -eq 0) { $env:NODE_ENV = 'production'; npm run build; $env:NODE_ENV = 'development' }; Pop-Location; if ($LASTEXITCODE -eq 0) { powershell -NoProfile -File tools\check-production-bundle.ps1 } }
Step 'Format' { cargo fmt --all -- --check }
Step 'Clippy' { cargo clippy --workspace --all-targets --locked -- -D warnings }
Step 'Tests' { cargo test --workspace --locked }
Step 'macOS compile check (crates without Apple tools)' { cargo clippy --target aarch64-apple-darwin -p tessera-scan -p tessera-insights -p tessera-cli -p tessera-treemap -p tessera-cleanup --all-targets --locked -- -D warnings }
if (-not $Quick) {
  $fx = Join-Path $root '.fixtures\check-20k'
  if (-not (Test-Path $fx)) { Step 'Fixture' { cargo run --release --locked -p tessera-cli -- gen $fx --files 20000 } }
  Step 'Memory gate' { cargo run --release --locked -p tessera-cli -- soak $fx --runs 10 --max-peak-growth-mib 8 }
  Step 'Screen request budgets' { cargo run --release --locked -p tessera-cli -- ui-bench $fx --check }
  # Disposable fixtures are removed afterwards (it has the .tessera-fixture marker).
  if (Test-Path (Join-Path $fx '.tessera-fixture')) { Remove-Item -Recurse -Force $fx }
}
Step 'Website build' { Push-Location website; npx astro build | Out-Null; Pop-Location }
if ($failed.Count) { Write-Host "`nFailed: $($failed -join ', ')" -ForegroundColor Red; exit 1 }
Write-Host "`nAll checks passed." -ForegroundColor Green
