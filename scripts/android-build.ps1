<#
  Builds the Android app (debug APK) on Windows.

  Why not just `npm run tauri -- android build`: Tauri links the Rust library into the Gradle project with a
  SYMBOLIC link, and Windows refuses that unless Developer Mode is on (or the account holds
  SeCreateSymbolicLinkPrivilege). This script lets Tauri compile the library (it sets the NDK compiler
  variables and the JNI names), copies the .so into the Gradle project when the link step fails, and runs Gradle
  itself. With Developer Mode on, Tauri's own build simply succeeds and the copy is a no-op.

    pwsh scripts/android-build.ps1                    # universal debug APK: arm64-v8a (phones, tablets) + x86_64 (emulator)
    pwsh scripts/android-build.ps1 -Abis arm64-v8a    # one ABI, smaller and faster
    pwsh scripts/android-build.ps1 -Release           # signed release APK when a keystore is configured; otherwise a debug build

  Release signing reads, in this order, the environment variables GT_KEYSTORE (path to the .jks / .keystore),
  GT_KEYSTORE_PASSWORD, GT_KEY_ALIAS, GT_KEY_PASSWORD (defaults to the keystore password), then the gitignored file
  app/src-tauri/gen/android/keystore.properties (storeFile=, storePassword=, keyAlias=, keyPassword=). The key is never
  stored in the repository. With -Release and no keystore this script warns and builds the debug APK instead.

  The APK lands in app/src-tauri/gen/android/app/build/outputs/apk/universal/<profile>/.
  Needs: rustup targets for the ABIs, the Android SDK + NDK 27, JDK 21 (ANDROID_HOME / NDK_HOME / JAVA_HOME are
  filled in from the usual places when unset, for this process only).
#>
param(
  [string[]]$Abis = @('arm64-v8a', 'x86_64'),
  [switch]$Release,
  # Keep the debug symbols in the .so (a debug library is ~380 MB per ABI with them; the APK is then ~760 MB).
  [switch]$KeepSymbols
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root 'app'
$android = Join-Path $app 'src-tauri/gen/android'

if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android/Sdk' }
if (-not $env:NDK_HOME) {
  $ndk = Get-ChildItem (Join-Path $env:ANDROID_HOME 'ndk') -Directory | Sort-Object Name -Descending | Select-Object -First 1
  if ($ndk) { $env:NDK_HOME = $ndk.FullName }
}
if (-not $env:JAVA_HOME) {
  $jbr = Get-ChildItem (Join-Path $env:USERPROFILE '.jdks') -Directory -Filter 'jbr-21*' -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($jbr) { $env:JAVA_HOME = $jbr.FullName }
}

$map = @{
  'arm64-v8a'   = @{ tauri = 'aarch64'; triple = 'aarch64-linux-android' }
  'armeabi-v7a' = @{ tauri = 'armv7'; triple = 'armv7-linux-androideabi' }
  'x86'         = @{ tauri = 'i686'; triple = 'i686-linux-android' }
  'x86_64'      = @{ tauri = 'x86_64'; triple = 'x86_64-linux-android' }
}
if ($Release) {
  $props = Join-Path $android 'keystore.properties'
  if (-not $env:GT_KEYSTORE -and -not (Test-Path $props)) {
    Write-Warning 'No release keystore (GT_KEYSTORE or keystore.properties): building the debug APK instead.'
    $Release = $false
  } elseif ($env:GT_KEYSTORE -and -not (Test-Path $env:GT_KEYSTORE)) {
    throw "GT_KEYSTORE points to a missing file: $($env:GT_KEYSTORE)"
  }
}
$profileName = if ($Release) { 'release' } else { 'debug' }

if (-not (Test-Path (Join-Path $android 'gradlew.bat'))) {
  Push-Location $app
  try { npm.cmd run tauri -- android init --ci } finally { Pop-Location }
}

foreach ($abi in $Abis) {
  $m = $map[$abi]
  if (-not $m) { throw "Unknown ABI $abi" }
  $lib = Join-Path $root "target/$($m.triple)/$profileName/libgt_app_lib.so"
  Push-Location $app
  try {
    $npmArgs = @('run', 'tauri', '--', 'android', 'build', '--apk', '--target', $m.tauri)
    if (-not $Release) { $npmArgs += '--debug' }
    $ErrorActionPreference = 'Continue'
    & npm.cmd $npmArgs 2>&1 | Tee-Object -Variable out | Out-Host
    $ErrorActionPreference = 'Stop'
  } finally { Pop-Location }
  if (-not (Test-Path $lib)) { throw "The Rust library for $abi was not built: $lib" }
  $dest = Join-Path $android "app/src/main/jniLibs/$abi"
  New-Item -ItemType Directory -Force $dest | Out-Null
  $out = Join-Path $dest 'libgt_app_lib.so'
  Copy-Item $lib $out -Force
  if (-not $KeepSymbols) {
    $strip = Join-Path $env:NDK_HOME 'toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-strip.exe'
    & $strip --strip-unneeded $out
  }
}

# Tauri's own Gradle tasks would call Tauri again (and fail on the link); the libraries are in place already.
$cap = $profileName.Substring(0, 1).ToUpper() + $profileName.Substring(1)
$skip = @('Universal', 'Arm64', 'Arm', 'X86', 'X86_64') | ForEach-Object { '-x'; "rustBuild$_$cap" }
# An APK rewritten in place keeps growing (it reached 797 MB): start from none. Libraries of ABIs not asked for would be packed too.
Get-ChildItem (Join-Path $android "app/build/outputs/apk/universal/$profileName") -Filter *.apk -ErrorAction SilentlyContinue | Remove-Item -Force
Push-Location $android
try {
  & .\gradlew.bat "assembleUniversal$cap" @skip --no-daemon
  if ($LASTEXITCODE -ne 0) { throw "Gradle failed (exit $LASTEXITCODE)" }
} finally { Pop-Location }
Get-ChildItem (Join-Path $android "app/build/outputs/apk/universal/$profileName") -Filter *.apk | ForEach-Object { "$($_.FullName)  $([math]::Round($_.Length/1MB,1)) MB" }
