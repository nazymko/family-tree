# Running the tests

Four commands, same behaviour on Windows (`scripts/test.ps1`) and macOS/Linux (`scripts/test.sh`). Both are thin wrappers
for `scripts/gate.mjs`, which runs independent steps at the same time, each to its own log. A failed step does not stop the
others; at the end it prints a summary and the log of every failure, and the exit code is non-zero if anything failed.

| Which command, when | Windows | macOS / Linux |
|---|---|---|
| while working: only what your changes can affect | `.\scripts\test.ps1 -Quick` | `bash scripts/test.sh --quick` |
| everyday: all Rust tests (nextest) and vitest | `.\scripts\test.ps1` | `bash scripts/test.sh` |
| window specs on the real app, several at a time | `.\scripts\test.ps1 -E2e [specs]` | `bash scripts/test.sh --e2e [specs]` |
| before merge: fmt, clippy, Rust tests, heavy tier, doctests, vitest, svelte-check, eslint | `.\scripts\test.ps1 -Full` | `bash scripts/test.sh --full` |
| the gate, then all window specs (changes nothing) | | `bash scripts/ship.sh` |
| the same, then merge develop into main, release build, install | | `bash scripts/ship.sh --merge "message"` |

Needs `cargo install cargo-nextest --locked` once (nextest settings: `.config/nextest.toml`).
Measured on a 16-thread machine, everything built: `--full` 158 s one step after another, 78 s now; the 10 window specs
172 s one after another, 39 to 45 s now; `--quick` on a one-file frontend change 7 s.

## `--quick`

Takes `git diff` against the merge-base with `develop` plus uncommitted and new files (`GT_QUICK_BASE=<ref>` compares
with another commit) and prints what it ran and what it skipped, so a green quick run is never mistaken for the full gate.

- A file of a crate: `cargo nextest` for that crate and everything that depends on it (`rdeps(...)`), and `cargo fmt --check`.
- `Cargo.toml`/`Cargo.lock`, `rust-toolchain.toml`, `.config/`, `tests/`, a `build.rs`, `tauri.conf.json`, capabilities, or any
  crate's `Cargo.toml`: all Rust tests.
- A file under `app/src` or `app/e2e-tauri`: `vitest related` (the tests that import it), eslint on the file, svelte-check.
- `package.json`, the lock file, vite/svelte/tsconfig/eslint configs, `*.html`, the i18n catalogues, the bindings, the test
  setup: vitest (all), svelte-check, eslint (all).
- Text only (`docs/`, `*.md`): nothing. `scripts/`: noted as covered by no test.
- A file it does not know: everything, on both sides. When unsure it runs more, never less.
- Never in quick: clippy, the heavy tier, doctests, the window specs.

## Window specs (the real app)

`bash scripts/test.sh --e2e` builds the test exe when it is older than its sources, then runs the specs in
`app/e2e-tauri/specs/` in parallel (`scripts/e2e-parallel.mjs`: `--jobs N`, default about a third of the threads, at most 6;
measured on 16 threads: 4 at a time 53 s, 6 at a time 39 s, 10 at a time 40 s, the slowest single spec is 35 s).
Specs are named without the suffix: `bash scripts/test.sh --e2e flows date-picker`. Screenshots go to `$GT_E2E_SHOTS`
(default: a folder in the run's temp folder, printed at the end). Needs `tauri-driver` and a matching `msedgedriver`
(`.tools/msedgedriver`, also found in the main checkout from a worktree).

- The test exe is built into its own folder, `target/e2e`, so `cargo test` and `clippy` can never replace it with a build
  that lacks the `custom-protocol` feature. Build by hand: `CARGO_TARGET_DIR=target/e2e npm --prefix app run tauri -- build --debug --no-bundle`.
- Each spec runs in its own wdio run, which is one isolated instance and can run next to your own Gene Tree and next to
  other runs: its own tauri-driver and native driver on ports of their own (not 4444), its own settings folder
  (`GT_E2E_CONFIG_DIR`: settings, recents, agent key hashes, the Anthropic key in a file, never the keychain), its own
  agent-access port (`GT_E2E_AGENT_PORT`), its own project folders, and a fresh settings folder for every spec file, so any
  spec can run alone. msedgedriver gives each session its own WebView2 profile folder. Ports are picked at random from
  41000-48999, outside Windows' dynamic range: a port from the OS was once taken by another program between picking and listening.
- With `GT_E2E=1` (debug builds only) the single-instance plugin is not registered, the shared temp folder is not swept, the
  keychain is not used, and windows start without taking the keyboard focus (`GT_E2E_FOCUS=1` turns that off). Without the
  variable the app behaves exactly as before.
- The synthetic file has 5,000 people for `flows`, `screenshots` and `agent-access` (they need a big tree); the other specs get
  1,000 (`GT_E2E_SMALL`).
- A run that is stopped (Ctrl+C, SIGTERM) kills its drivers and apps. If the script is killed hard, a small reaper
  process (`scripts/lib/reaper.mjs`) kills them a moment later. Nothing is ever stopped by name, only by PID.

## `ship.sh`

Runs `--full`, then the window specs (side by side the CPU load made windows miss their timeouts), stops at the first failure and leaves `main` alone.
With `--merge "message"` and both green it merges `develop` into a scratch checkout of `main`, does the release build into
`target/ship` (never the folder of a running exe) and installs it; only then does `main` move. It refuses to start while
another `ship.sh` runs (lock file with the PID, a lock of a dead process is cleared), needs a clean tree and HEAD on the tip
of `develop`, and never pushes.

## Installed copy

`.\scripts\install-app.ps1 [-Shortcut]` copies `target\release\gene-tree.exe` to `%LOCALAPPDATA%\GeneTree\gene-tree.exe` while
that copy is running: the running exe is renamed to `gene-tree.exe.old-<time>` (Windows allows that), the new one is put in
its place, old copies no longer in use are deleted. `-Shortcut` adds a "Gene Tree" Start Menu shortcut. Settings and data
follow the app identifier (`app.gene-tree.desktop`), not the exe, so they are the same.

## The three kinds of Rust test

1. Normal tests: run every time.
2. Heavy tier: a few tests that take 5 to 50 s in a debug build (big generated trees, a 512 MiB decompression bomb,
   random edit/undo sequences). They are `#[ignore = "heavy: runs before merge"]` and their name starts with `heavy_`.
   The prefix is how the tier is selected without also pulling in the release-only budgets (which are ignored too and
   fail in a debug build):
   `cargo nextest run --workspace --run-ignored only -E 'test(/heavy_/)'`, or with plain cargo
   `cargo test --workspace -- --ignored heavy_`.
   CI runs the second form on every push. When you add a slow test, give it the `heavy_` prefix and that ignore text.
   Security tests (the Gramps decompression bomb) stay in this tier and are never dropped.
3. Release-only budgets: `#[ignore = "release-only budget check"]`, wall-clock budgets that only mean something in a
   release build: `cargo test --release -p gt-session --test budget -- --ignored --nocapture` (and the same for
   the other crates' `perf` tests).

Doctests: nextest does not run them, so the full run adds `cargo test --doc --workspace` (there are none at the moment).

## Notes

- No test depends on a real port being free: the agent-access tests choose their own free port (a test that proves "nothing
  listens" does it on a port nobody holds, not on 8765).
- Frontend tests use `pool: 'vmThreads'` (one jsdom per worker, files still isolated). `isolate: false` was measured
  flaky; do not use it.
- Clippy in a separate target folder was measured against sharing `target/` (124 s both ways after touching a core crate), so
  it shares.
