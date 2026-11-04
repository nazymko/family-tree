<#
.SYNOPSIS
  Builds Gene Tree in release mode on Windows and copies the shareable files into dist-release/.
.DESCRIPTION
  Produces
    dist-release/Gene-Tree-<version>-windows-x64.exe          portable exe, no install (needs WebView2, built into Windows 10/11)
    dist-release/Gene-Tree-<version>-windows-x64-setup.exe    NSIS installer (alternative)
  Works in PowerShell 7 and Windows PowerShell 5.1. Run from anywhere: .\scripts\build-release.ps1
.PARAMETER Msi
  Also build the .msi installer (downloads WiX on first use).
#>
[CmdletBinding()]
param([switch]$Msi)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root 'app'
$out = Join-Path $root 'dist-release'

function Fail($msg) { Write-Host "ERROR: $msg" -ForegroundColor Red; exit 1 }

# --- prerequisites ---------------------------------------------------------
$missing = @()
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $missing += 'Rust (cargo) - install from https://rustup.rs' }
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { $missing += 'Node.js 20 or newer - install from https://nodejs.org' }
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { $missing += 'npm (comes with Node.js)' }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Get-Command link.exe -ErrorAction SilentlyContinue) -and -not (Test-Path $vswhere)) {
  $missing += 'Visual Studio Build Tools with the "Desktop development with C++" workload - https://visualstudio.microsoft.com/visual-cpp-build-tools/'
}
if ($missing.Count -gt 0) {
  Write-Host 'Cannot build yet, missing:' -ForegroundColor Red
  $missing | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
  exit 1
}

$conf = Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$version = $conf.version
$product = ($conf.productName -replace '\s+', '-')

if (-not (Test-Path (Join-Path $app 'node_modules'))) {
  Write-Host 'Installing frontend packages (npm ci)...'
  Push-Location $app
  try { & npm.cmd ci } finally { Pop-Location }
  if ($LASTEXITCODE -ne 0) { Fail 'npm ci failed' }
}

# --- build -----------------------------------------------------------------
$bundles = if ($Msi) { 'nsis,msi' } else { 'nsis' }
Write-Host "Building $product $version (release, bundles: $bundles). The first build takes several minutes..."
Push-Location $app
try { & npm.cmd run tauri -- build --bundles $bundles } finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { Fail 'tauri build failed, see the output above' }

# --- collect ---------------------------------------------------------------
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$exe = Join-Path $targetDir 'release\gene-tree.exe'
if (-not (Test-Path $exe)) { Fail "release exe not found at $exe" }

New-Item -ItemType Directory -Force $out | Out-Null
$files = @()
$portable = Join-Path $out "$product-$version-windows-x64.exe"
Copy-Item $exe $portable -Force
$files += $portable

$nsis = Get-ChildItem (Join-Path $targetDir 'release\bundle\nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($nsis) {
  $dst = Join-Path $out "$product-$version-windows-x64-setup.exe"
  Copy-Item $nsis.FullName $dst -Force
  $files += $dst
} else { Write-Host 'Warning: NSIS setup exe not found.' -ForegroundColor Yellow }

if ($Msi) {
  $m = Get-ChildItem (Join-Path $targetDir 'release\bundle\msi') -Filter '*.msi' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($m) {
    $dst = Join-Path $out "$product-$version-windows-x64.msi"
    Copy-Item $m.FullName $dst -Force
    $files += $dst
  }
}

Write-Host ''
Write-Host 'Done. Files to share:' -ForegroundColor Green
foreach ($f in $files) {
  Write-Host ('{0,8:N1} MB  {1}' -f ((Get-Item $f).Length / 1MB), $f)
}
Write-Host 'The portable .exe runs without install on Windows 10/11 (WebView2 is built in).'
