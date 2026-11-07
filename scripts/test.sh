#!/usr/bin/env bash
# Runs the Gene Tree tests. The work is done by scripts/gate.mjs (independent steps run at the same time, each to its own
# log; a failed step does not stop the others; the exit code is non-zero if anything failed).
#
#   bash scripts/test.sh                  everyday: Rust tests (nextest) and the frontend tests (vitest), side by side
#   bash scripts/test.sh --quick          only what your changes can affect (seconds for one frontend file); says what it skipped
#   bash scripts/test.sh --full           before merge: fmt, clippy, Rust tests, heavy tier, doctests, vitest, svelte-check, eslint
#   bash scripts/test.sh --e2e [spec...]  window specs on the real app, several at a time (builds the test exe first if stale)
#
# The release-only budgets (#[ignore = "release-only budget check"]) are in no run; see docs/testing.md.
# Needs cargo-nextest: cargo install cargo-nextest --locked
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v node >/dev/null || { echo "Node.js is missing (it is needed for the app anyway)." >&2; exit 1; }
exec node "$ROOT/scripts/gate.mjs" "$@"
