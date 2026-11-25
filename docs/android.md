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
The script honours `CARGO_TARGET_DIR` (e.g. `E:/gene-tree-target/android`) when it looks for the built library, so Rust
output can live on another drive; no Developer Mode is needed because the symlink failure is ignored and the library is copied.

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
- A phone is a "compact" screen: coarse pointer and (width up to 940 px or height up to 500 px), see
  `app/src/lib/ui/compact.svelte.ts`. There the chart toolbars show icons with a "..." overflow (zoom lives in
  pinch), the minimap is hidden, dialogs are full-screen sheets and context menus are clamped into the visible
  viewport (opening upward when the bottom has no room). A desktop window never matches.
- The Android status and navigation bars are coloured from the page theme: `theme.ts` calls the `GeneTreeBars` bridge
  (`MainActivity.kt`), which sets the bar colours and the light/dark icon style. The same bridge gives the picked
  document's real name (`displayName`), so the import wizard and the staged file show `family.ged`, not `msf:20`.

## Signed release build

`pwsh scripts/android-build.ps1 -Release` builds a release APK. The signing key is read from environment variables
(`GT_KEYSTORE` = path, `GT_KEYSTORE_PASSWORD`, `GT_KEY_ALIAS`, `GT_KEY_PASSWORD`) or from a `keystore.properties` file in
`app/src-tauri/gen/android/` (`storeFile`, `storePassword`, `keyAlias`, `keyPassword`; the file is gitignored). Without a
key the script warns and builds the debug APK instead; a `GT_KEYSTORE` that points to a missing file is an error. Never
commit a keystore or its passwords. Make a key once with `keytool -genkeypair -v -keystore gene-tree.jks -alias gene-tree
-keyalg RSA -keysize 2048 -validity 10000` and keep it (and its passwords) outside the repository.

## Process exit (investigated)

Earlier builds aborted with `pthread_mutex_lock called on a destroyed mutex` (wry `main_pipe.rs` SendError) when the
activity ended: returning from `run` lets the C runtime run `exit()` handlers while wry's worker threads still hold
global mutexes. There is no clean fix without patching wry, so the workaround stays: Back moves the task to the
background instead of finishing the activity, the open tree's WAL is checkpointed on Suspended, and the process ends
itself with SIGKILL once the tree is closed (`mobile::end_process`). Checked on the emulator (Android 16): Back, closing
the tree, and swiping the app out of Recents leave no tombstone and no abort in logcat (the system reports
`exited due to signal 9`, which is the normal fate of a swiped-away app).
