<#
.SYNOPSIS
  Runs the Family Tree tests (the same as scripts/test.sh; the work is done by scripts/gate.mjs).
.DESCRIPTION
  .\scripts\test.ps1          everyday: Rust tests (nextest) and the frontend tests, side by side
  .\scripts\test.ps1 -Quick   only what your changes can affect; says what it skipped
  .\scripts\test.ps1 -Full    before merge: fmt, clippy, Rust tests, heavy tier, doctests, vitest, svelte-check, eslint
  .\scripts\test.ps1 -E2e [spec names]   window specs on the real app, several at a time
  Independent steps run at the same time, each to its own log. Exit code: 0 only when every step passed.
  Needs cargo-nextest: cargo install cargo-nextest --locked
#>
[CmdletBinding()]
param([switch]$Full, [switch]$Quick, [switch]$E2e, [Parameter(ValueFromRemainingArguments)][string[]]$Specs)
$gate = Join-Path $PSScriptRoot 'gate.mjs'
$mode = if ($Full) { @('--full') } elseif ($Quick) { @('--quick') } elseif ($E2e) { @('--e2e') + $Specs } else { @() }
& node $gate @mode
exit $LASTEXITCODE
