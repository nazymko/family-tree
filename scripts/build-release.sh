#!/usr/bin/env bash
# Builds Gene Tree in release mode on macOS or Linux and copies the shareable file into dist-release/.
#   macOS : dist-release/Gene-Tree-<version>-macos-universal.dmg  (aarch64 + x86_64)
#   Linux : dist-release/Gene-Tree-<version>-linux-<arch>.AppImage
# Run from anywhere: bash scripts/build-release.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/app"
OUT="$ROOT/dist-release"
OS="$(uname -s)"
ARCH="$(uname -m)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

missing=()
command -v cargo >/dev/null || missing+=("Rust (cargo) - install from https://rustup.rs")
command -v node >/dev/null || missing+=("Node.js 20 or newer - https://nodejs.org")
command -v npm >/dev/null || missing+=("npm (comes with Node.js)")

case "$OS" in
  Darwin)
    xcode-select -p >/dev/null 2>&1 || missing+=("Xcode command line tools - run: xcode-select --install")
    ;;
  Linux)
    hint="On Debian/Ubuntu: sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf"
    if command -v pkg-config >/dev/null; then
      for p in webkit2gtk-4.1 gtk+-3.0 librsvg-2.0; do
        pkg-config --exists "$p" || missing+=("system library $p. $hint")
      done
    else
      missing+=("pkg-config and the webview libraries. $hint")
    fi
    command -v patchelf >/dev/null || missing+=("patchelf (sudo apt-get install -y patchelf)")
    ;;
  *) echo "Unsupported OS '$OS'. On Windows use scripts/build-release.ps1." >&2; exit 1 ;;
esac

if [ ${#missing[@]} -gt 0 ]; then
  echo "Cannot build yet, missing:" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  exit 1
fi

VERSION="$(node -p "require('$APP/src-tauri/tauri.conf.json').version")"
PRODUCT="Gene-Tree"

if [ ! -d "$APP/node_modules" ]; then
  echo "Installing frontend packages (npm ci)..."
  npm --prefix "$APP" ci
fi

mkdir -p "$OUT"
echo "Building $PRODUCT $VERSION (release). The first build takes several minutes..."

if [ "$OS" = "Darwin" ]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  npm --prefix "$APP" run tauri -- build --target universal-apple-darwin --bundles dmg
  SRC="$(ls -t "$TARGET_DIR"/universal-apple-darwin/release/bundle/dmg/*.dmg | head -n1)"
  DST="$OUT/$PRODUCT-$VERSION-macos-universal.dmg"
else
  # Lets the AppImage tools run where FUSE is unavailable (containers, CI).
  export APPIMAGE_EXTRACT_AND_RUN=1
  npm --prefix "$APP" run tauri -- build --bundles appimage
  SRC="$(ls -t "$TARGET_DIR"/release/bundle/appimage/*.AppImage | head -n1)"
  DST="$OUT/$PRODUCT-$VERSION-linux-$ARCH.AppImage"
fi

cp -f "$SRC" "$DST"
[ "$OS" = "Linux" ] && chmod +x "$DST"
echo
echo "Done. File to share:"
du -h "$DST" | awk '{print "  " $1 "  " $2}'
