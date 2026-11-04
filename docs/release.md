# Building a shareable app

One file per operating system. A machine cannot cross-build the other systems in practice, so build each on its own OS
(locally with the scripts, or on GitHub with the workflow). All builds are release builds and end up in `dist-release/`
(git-ignored).

| OS | File | Notes |
|---|---|---|
| Windows | `Gene-Tree-<version>-windows-x64.exe` | Portable: the release exe, no install. Also `...-setup.exe` (NSIS installer). |
| macOS | `Gene-Tree-<version>-macos-universal.dmg` | Universal: Apple Silicon and Intel. |
| Linux | `Gene-Tree-<version>-linux-x86_64.AppImage` | `chmod +x` and run. |

## Build locally

You need Rust (rustup, toolchain from `rust-toolchain.toml`), Node.js 20+ and npm. The scripts check this and say what is
missing; they run `npm ci` when `app/node_modules` does not exist.

- Windows (PowerShell 7 or Windows PowerShell 5.1): `.\scripts\build-release.ps1` (add `-Msi` for an .msi as well).
  Needs the Visual Studio Build Tools with the C++ workload. The first run downloads NSIS (needs internet).
- macOS: `xcode-select --install`, then `bash scripts/build-release.sh`.
- Linux: `sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf`,
  then `bash scripts/build-release.sh`.

The first build takes several minutes (the release profile compiles every dependency).

## Build on GitHub

The repo has no remote yet. After creating one and pushing:

1. Open the repository on GitHub, Actions, **Release**, **Run workflow**. Windows, macOS and Linux build in parallel; each
   uploads its file as an artifact (`gene-tree-Windows`, `gene-tree-macOS`, `gene-tree-Linux`) on the run page.
2. Or push a tag: `git tag v0.1.0 && git push origin v0.1.0`. Besides the artifacts this creates a **draft** GitHub Release
   `v0.1.0` with all files attached. Review it and press Publish, then share the release link.

The version in the file names comes from `app/src-tauri/tauri.conf.json`; bump it before tagging.

## First-run warnings (the builds are unsigned)

- Windows SmartScreen: "Windows protected your PC" - click **More info**, then **Run anyway**.
- macOS: Gatekeeper refuses the app. Right-click the app, **Open**, **Open** again; or run
  `xattr -dr com.apple.quarantine "/Applications/Gene Tree.app"`.
- Linux: no warning; make the file executable first.

Signing would need: a Windows code-signing certificate (OV/EV, or Azure Trusted Signing); on macOS an Apple Developer
account (99 USD/year) with a Developer ID certificate plus notarization. Tauri reads these from environment variables
(`APPLE_CERTIFICATE`, `APPLE_ID`, `WINDOWS_CERTIFICATE`, ...), which would be added as repository secrets for the workflow.

## Windows: WebView2

The app draws with Microsoft Edge WebView2. It is part of Windows 11 and of current Windows 10 updates, so the portable
exe runs as is. On an old Windows 10 without it, install the "Evergreen Bootstrapper" from Microsoft; the NSIS setup
exe downloads it automatically when missing.

## Single instance

The app allows only one running copy. Starting a second copy (for example a release exe while a debug build is open)
hands over to the first one and exits immediately.
