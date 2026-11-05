# Running the tests

Two commands, same behaviour on Windows (`scripts/test.ps1`) and macOS/Linux (`scripts/test.sh`):

| | Windows | macOS / Linux | What it runs |
|---|---|---|---|
| everyday | `.\scripts\test.ps1` | `bash scripts/test.sh` | all Rust tests except the heavy tier and the release-only budgets (cargo nextest), then the frontend tests (vitest) |
| before merge | `.\scripts\test.ps1 -Full` | `bash scripts/test.sh --full` | `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, the everyday Rust tests, the heavy tier, `cargo test --doc`, vitest, svelte-check, eslint |

The full run keeps going after a failed step, prints the failed steps at the end and exits non-zero if any step failed.
Needs `cargo install cargo-nextest --locked` once (nextest settings: `.config/nextest.toml`).

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

- `a_new_state_asks_for_no_key_and_access_is_still_off` expects nothing to listen on port 8765: it fails while the
  app (or anything else) has agent access switched on at that default port.
- Frontend tests use `pool: 'vmThreads'` (one jsdom per worker, files still isolated). `isolate: false` was measured
  flaky; do not use it.
