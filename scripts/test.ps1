<#
.SYNOPSIS
  Runs the Gene Tree tests. Everyday run by default, the full before-merge run with -Full.
.DESCRIPTION
  Everyday (about 40 s warm): every Rust test except the heavy tier and the release-only budgets (cargo nextest),
  then the frontend tests (vitest).
  -Full (a few minutes): everything below, and it keeps going after a failed step so one run shows every problem:
    cargo fmt --check, cargo clippy --all-targets -D warnings,
    the everyday Rust tests, the heavy tier (the tests named heavy_*, ignored by default),
    cargo test --doc, vitest, svelte-check (npm run check), eslint (npm run lint).
  The release-only budgets (#[ignore = "release-only budget check"]) are NOT part of either run: they need
  `cargo test --release -- --ignored`. See docs/testing.md.
  Exit code: 0 only when every step passed. Works in PowerShell 7 and Windows PowerShell 5.1.
  Needs cargo-nextest: cargo install cargo-nextest --locked
.PARAMETER Full
  Run the full before-merge set instead of the everyday one.
#>
[CmdletBinding()]
param([switch]$Full)

# Native tools write progress to stderr; with 'Stop' that would abort the script. Exit codes are checked by hand.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root 'app'
$failed = New-Object System.Collections.Generic.List[string]
$timings = New-Object System.Collections.Generic.List[string]

if (-not (Get-Command cargo-nextest -ErrorAction SilentlyContinue)) {
  Write-Host 'cargo-nextest is missing. Install it once with: cargo install cargo-nextest --locked' -ForegroundColor Red
  exit 1
}
if (-not (Test-Path (Join-Path $app 'node_modules'))) {
  Write-Host 'app/node_modules is missing. Run: npm --prefix app ci' -ForegroundColor Red
  exit 1
}

function Step([string]$Name, [string]$Dir, [scriptblock]$Cmd) {
  Write-Host ''
  Write-Host "=== $Name" -ForegroundColor Cyan
  $sw = [Diagnostics.Stopwatch]::StartNew()
  Push-Location $Dir
  try { & $Cmd; $code = $LASTEXITCODE } finally { Pop-Location }
  if ($null -eq $code) { $code = 1 }
  $timings.Add(('{0,6:N1} s  {1}  (exit {2})' -f $sw.Elapsed.TotalSeconds, $Name, $code))
  if ($code -ne 0) {
    $failed.Add($Name)
    Write-Host "FAILED: $Name (exit $code)" -ForegroundColor Red
  }
}

if ($Full) {
  Step 'cargo fmt --check' $root { cargo fmt --all -- --check }
  Step 'cargo clippy' $root { cargo clippy -q --workspace --all-targets -- -D warnings }
}
Step 'Rust tests (nextest, everyday set)' $root { cargo nextest run --workspace --no-fail-fast }
if ($Full) {
  Step 'Rust heavy tier (heavy_*)' $root { cargo nextest run --workspace --no-fail-fast --run-ignored only -E 'test(/heavy_/)' }
  Step 'Rust doctests' $root { cargo test --doc --workspace }
}
Step 'Frontend tests (vitest)' $app { npx.cmd --no-install vitest run }
if ($Full) {
  Step 'svelte-check' $app { npm.cmd run check }
  Step 'eslint' $app { npm.cmd run lint }
}

Write-Host ''
Write-Host '--- summary' -ForegroundColor Cyan
$timings | ForEach-Object { Write-Host $_ }
if ($failed.Count -gt 0) {
  Write-Host ''
  Write-Host ('FAILED steps: ' + ($failed -join '; ')) -ForegroundColor Red
  exit 1
}
Write-Host 'All steps passed.' -ForegroundColor Green
exit 0
