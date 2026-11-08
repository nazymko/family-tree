# Builds and releases

## The release cycle

```
 feature branch ──PR──> develop ──(tests pass: scripts/ship.sh)──> main ──┬── every night it changed ──> "nightly" pre-release
                                                                          └── 1st of every month ─────> vX.Y.Z release
```

| What | When | Workflow | Result |
|---|---|---|---|
| **CI** | Every push and pull request, and nightly | `ci.yml` | fmt, clippy, Rust tests on Windows, macOS and Linux; frontend checks; a debug build on each system. The window specs (e2e) run nightly. |
| **Nightly** | 02:30 UTC each night `main` has new commits (or "Run workflow") | `nightly.yml` | The `nightly` pre-release is replaced by a build of `main`: `Family-Tree-nightly-<date>-<sha>-...`. |
| **Release** | The 1st of every month, 06:00 UTC (or "Run workflow" on *Prepare release*) | `prepare-release.yml` then `release.yml` | When CI passed on `main` and something new landed since the last release: tag `vX.Y.Z`, build all three systems, publish a GitHub Release with notes and `SHA256SUMS.txt`. |
| **Hand-made release** | `git tag v0.2.1 origin/main && git push origin v0.2.1` | `release.yml` | The same release for that tag; for a quick patch between cycles. |

**Versions.** `prepare-release` picks the next version from the commits since the last tag: a `feat` commit raises the
minor version (0.2.0 → 0.3.0), otherwise only fixes raise the patch version (0.2.0 → 0.2.1). "Run workflow" can force
`patch`, `minor` or `major`. The first release ships the version in `app/src-tauri/tauri.conf.json` (0.1.0).

**Nothing commits for a release.** The tag is the release. The build writes the version into `Cargo.toml`,
`tauri.conf.json` and `package.json` only in its own copy (`node scripts/release.mjs version X.Y.Z`), so the local
`develop` and `main` never fall behind GitHub. Release notes come from the `feat`, `fix` and `perf` commit messages
between the previous tag and this one (`node scripts/release.mjs notes vX.Y.Z`), which is why those messages are written
for users (CONTRIBUTING.md).

**A red `main` is not released.** `prepare-release` stops when the latest CI run on `main`'s head did not pass; fix it
and press "Run workflow" on *Prepare release*.

Try the scripts locally:

```bash
node scripts/release.mjs current       # 0.1.0
node scripts/release.mjs next          # the version the next cycle would release (empty: nothing new)
node scripts/release.mjs notes v0.2.0  # the notes of a tag
```

## Files

One file per operating system; each system builds its own.

| OS | File | Notes |
|---|---|---|
| Windows | `Family-Tree-<version>-windows-x64-setup.exe` | NSIS installer. Also `...-portable.exe`: the release exe, no install. |
| macOS | `Family-Tree-<version>-macos-universal.dmg` | Universal: Apple Silicon and Intel. |
| Linux | `Family-Tree-<version>-linux-x86_64.AppImage` | `chmod +x` and run. |

## Build locally

You need Rust (rustup, toolchain from `rust-toolchain.toml`), Node.js 20+ and npm. The scripts check this and say what is
missing; they run `npm ci` when `app/node_modules` does not exist. Output goes to `dist-release/` (git-ignored).

- Windows (PowerShell 7 or Windows PowerShell 5.1): `.\scripts\build-release.ps1` (add `-Msi` for an .msi as well).
  Needs the Visual Studio Build Tools with the C++ workload. The first run downloads NSIS (needs internet).
- macOS: `xcode-select --install`, then `bash scripts/build-release.sh`.
- Linux: `sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf`,
  then `bash scripts/build-release.sh`.

The first build takes several minutes (the release profile compiles every dependency). On the owner's machine
`bash scripts/ship.sh --merge "msg"` runs the gate and the window specs, merges `develop` into `main`, builds and installs
the app to `%LOCALAPPDATA%\GeneTree` (the desktop shortcut points there).

## First-run warnings (the builds are unsigned)

- Windows SmartScreen: "Windows protected your PC": click **More info**, then **Run anyway**.
- macOS: Gatekeeper refuses the app. Right-click the app, **Open**, **Open** again; or run
  `xattr -dr com.apple.quarantine "/Applications/Family Tree.app"`.
- Linux: no warning; make the file executable first.

Signing would need: a Windows code-signing certificate (OV/EV, or Azure Trusted Signing); on macOS an Apple Developer
account (99 USD/year) with a Developer ID certificate plus notarization. Tauri reads these from environment variables
(`APPLE_CERTIFICATE`, `APPLE_ID`, `WINDOWS_CERTIFICATE`, ...), which would be added as repository secrets and passed to
the build step in `build.yml`.

## Windows: WebView2

The app draws with Microsoft Edge WebView2. It is part of Windows 11 and of current Windows 10 updates, so the portable
exe runs as is. On an old Windows 10 without it, install the "Evergreen Bootstrapper" from Microsoft; the NSIS setup
exe downloads it automatically when missing.

## Single instance

The app allows only one running copy. Starting a second copy (for example a release exe while a debug build is open)
hands over to the first one and exits immediately.
