# Family Tree on Android (phones and tablets)

The Android app is the same Tauri app, built for Android. It is local-first like the desktop app: the tree is a folder
(`tree.db` + `media/`) in the app's own storage, there is no server.

## Build a debug APK (Windows)

Needs: the Android SDK with NDK 27 (`ANDROID_HOME`, `NDK_HOME`), JDK 21 (`JAVA_HOME`), and the Rust targets:

```
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
pwsh scripts/android-build.ps1                  # arm64-v8a (phones, tablets) + x86_64 (emulator)
pwsh scripts/android-build.ps1 -Abis arm64-v8a  # one ABI
```

The APK is `app/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`.
`npm --prefix app run tauri -- android build --debug --apk` also works when Windows Developer Mode is on (Tauri links
the library with a symbolic link, which Windows refuses otherwise; the script copies the library instead).

Install and start: `adb install -r <apk>` then `adb shell am start -n app.gene_tree.desktop/.MainActivity`.

## What differs from the desktop app

- Trees live in `<app data>/trees/<name>`. There is no folder picker: a new tree needs only a name, a GEDCOM or
  project file is picked with the system file picker and copied into the app first (`mobile_stage_pick`).
- Export, "Send a copy" and chart pictures are written into the app's cache and then offered through the system save
  dialog (`mobile_deliver`), so a project zip can travel back to the desktop and be merged with "Merge a relative's
  copy". "Open a received copy" unpacks beside the other trees.
- Not offered on a phone: opening a tree from a folder, saving a copy into a folder, merging from a folder, photo
  folders when importing, "show in folder", and the local agent (MCP) server.
- Below 940 px wide the sidebar is a drawer behind a menu button. A long press on a person's box opens the same menu as
  a right-click; two quick taps on a chart recentre; a tap in a list opens the item.
