#!/usr/bin/env bash
# Runs the Gene Tree tests. Everyday run by default, the full before-merge run with --full.
#   everyday: every Rust test except the heavy tier and the release-only budgets (cargo nextest), then vitest.
#   --full  : cargo fmt --check, clippy -D warnings, the everyday Rust tests, the heavy tier (tests named
#             heavy_*, ignored by default), cargo test --doc, vitest, svelte-check, eslint. It keeps going after
#             a failed step, prints every failure at the end and exits non-zero if any step failed.
# The release-only budgets (#[ignore = "release-only budget check"]) are in neither run; see docs/testing.md.
# Needs cargo-nextest: cargo install cargo-nextest --locked
# Run from anywhere: bash scripts/test.sh [--full]
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/app"
FULL=0
case "${1:-}" in
  "") ;;
  --full) FULL=1 ;;
  *) echo "usage: bash scripts/test.sh [--full]" >&2; exit 2 ;;
esac

command -v cargo-nextest >/dev/null || { echo "cargo-nextest is missing. Install it once with: cargo install cargo-nextest --locked" >&2; exit 1; }
[ -d "$APP/node_modules" ] || { echo "app/node_modules is missing. Run: npm --prefix app ci" >&2; exit 1; }

FAILED=()
TIMINGS=()

# step <name> <dir> <command...>: runs the command, records its exit code (no pipes, so the code is the tool's own).
step() {
  local name="$1" dir="$2"
  shift 2
  echo
  echo "=== $name"
  local start=$SECONDS code
  (cd "$dir" && "$@")
  code=$?
  TIMINGS+=("$((SECONDS - start)) s  $name  (exit $code)")
  if [ "$code" -ne 0 ]; then
    FAILED+=("$name")
    echo "FAILED: $name (exit $code)" >&2
  fi
}

if [ "$FULL" = 1 ]; then
  step "cargo fmt --check" "$ROOT" cargo fmt --all -- --check
  step "cargo clippy" "$ROOT" cargo clippy -q --workspace --all-targets -- -D warnings
fi
step "Rust tests (nextest, everyday set)" "$ROOT" cargo nextest run --workspace --no-fail-fast
if [ "$FULL" = 1 ]; then
  step "Rust heavy tier (heavy_*)" "$ROOT" cargo nextest run --workspace --no-fail-fast --run-ignored only -E 'test(/heavy_/)'
  step "Rust doctests" "$ROOT" cargo test --doc --workspace
fi
step "Frontend tests (vitest)" "$APP" npx --no-install vitest run
if [ "$FULL" = 1 ]; then
  step "svelte-check" "$APP" npm run check
  step "eslint" "$APP" npm run lint
fi

echo
echo "--- summary"
printf '%s\n' "${TIMINGS[@]}"
if [ ${#FAILED[@]} -gt 0 ]; then
  echo
  printf 'FAILED steps:\n'
  printf '  - %s\n' "${FAILED[@]}" >&2
  exit 1
fi
echo "All steps passed."
