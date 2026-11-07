<#
.SYNOPSIS
  Puts a release build of Gene Tree in %LOCALAPPDATA%\GeneTree\gene-tree.exe, also while that copy is running.
.DESCRIPTION
  The owner runs the INSTALLED copy, so a build never meets a locked target\release\gene-tree.exe.
  Windows lets a running exe be renamed but not overwritten or deleted, so the swap is:
    1. the new exe is copied next to the old one under a temporary name,
    2. the old exe is renamed to gene-tree.exe.old-<time>,
    3. the temporary copy is renamed to gene-tree.exe (if that fails, the old one is put back),
    4. every gene-tree.exe.old-* that is no longer in use is deleted (one still running stays until the next install).
  The running copy keeps running on its old file; the new one starts the next time the app is opened.
  Nothing else is touched. The data (settings, recent list, WebView2 profile) lives under the app identifier
  (app.gene-tree.desktop in tauri.conf.json: %APPDATA%\app.gene-tree.desktop and %LOCALAPPDATA%\app.gene-tree.desktop),
  not next to the exe, so a new exe in a new place sees the same data.
  The script never starts or stops the app.
.PARAMETER Source
  The exe to install. Default: target\release\gene-tree.exe of this checkout.
.PARAMETER Dest
  The install folder. Default: %LOCALAPPDATA%\GeneTree. (Tests use a scratch folder.)
.PARAMETER Shortcut
  Also create the Start Menu shortcut "Gene Tree" that points at the installed exe.
.PARAMETER ShortcutDir
  Where the shortcut goes. Default: the user's Start Menu Programs folder. (Tests use a scratch folder.)
#>
[CmdletBinding()]
param(
  [string]$Source,
  [string]$Dest,
  [switch]$Shortcut,
  [string]$ShortcutDir
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Source) { $Source = Join-Path $root 'target\release\gene-tree.exe' }
if (-not $Dest) { $Dest = Join-Path $env:LOCALAPPDATA 'GeneTree' }
if (-not $ShortcutDir) { $ShortcutDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs' }

if (-not (Test-Path -LiteralPath $Source)) { throw "The exe to install is missing: $Source. Build it first (scripts/build-release.ps1)." }
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$target = Join-Path $Dest 'gene-tree.exe'

$same = (Test-Path -LiteralPath $target) -and ((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash)
if ($same) {
  Write-Host "Already installed and identical: $target"
} else {
  $stage = Join-Path $Dest ("gene-tree.exe.new-{0}" -f $PID)
  Copy-Item -LiteralPath $Source -Destination $stage -Force
  $parked = $null
  try {
    if (Test-Path -LiteralPath $target) {
      $parked = '{0}.old-{1:yyyyMMddHHmmssfff}' -f $target, (Get-Date)
      # Allowed while the exe runs. Fails only when something forbids the rename; then nothing has changed yet.
      Move-Item -LiteralPath $target -Destination $parked
    }
    Move-Item -LiteralPath $stage -Destination $target
  } catch {
    if ($parked -and (Test-Path -LiteralPath $parked) -and -not (Test-Path -LiteralPath $target)) { Move-Item -LiteralPath $parked -Destination $target }
    Remove-Item -LiteralPath $stage -Force -ErrorAction SilentlyContinue
    throw
  }
  Write-Host "Installed: $target"
  if ($parked) { Write-Host "The previous exe was renamed to $(Split-Path -Leaf $parked)." }
}

# Old copies: delete what is no longer in use; a running one refuses and stays for the next install.
$left = 0
Get-ChildItem -LiteralPath $Dest -Filter 'gene-tree.exe.old-*' -File -ErrorAction SilentlyContinue | ForEach-Object {
  try { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop } catch { $left++ }
}
Get-ChildItem -LiteralPath $Dest -Filter 'gene-tree.exe.new-*' -File -ErrorAction SilentlyContinue | ForEach-Object {
  # Left by an install that was interrupted; this install has finished, so nobody else is using them.
  if ($_.Name -ne ("gene-tree.exe.new-{0}" -f $PID)) { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue }
}
if ($left -gt 0) { Write-Host "$left old copy(ies) still in use stay until the next install." }

if ($Shortcut) {
  New-Item -ItemType Directory -Force -Path $ShortcutDir | Out-Null
  $lnk = Join-Path $ShortcutDir 'Gene Tree.lnk'
  $shell = New-Object -ComObject WScript.Shell
  $s = $shell.CreateShortcut($lnk)
  $s.TargetPath = $target
  $s.WorkingDirectory = $Dest
  $s.IconLocation = "$target,0"
  $s.Description = 'Gene Tree'
  $s.Save()
  Write-Host "Shortcut: $lnk"
}
