#!/usr/bin/env bash
# bash scripts/ship.sh                    gate (--full) and all window specs at once. Changes nothing.
# bash scripts/ship.sh --merge "message"  the same, then merge develop into main, release build, install (see scripts/ship.mjs)
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec node "$ROOT/scripts/ship.mjs" "$@"
