#!/usr/bin/env bash
# Fetch third-party GEDCOM test files into tests/corpus/ (gitignored). Needs network.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/tests/corpus"
mkdir -p "$dest"
if [ ! -d "$dest/gedcom7code" ]; then
  git clone --depth 1 https://github.com/gedcom7code/test-files "$dest/gedcom7code"
fi
if [ ! -d "$dest/familysearch" ]; then
  git clone --depth 1 --filter=blob:none --sparse https://github.com/FamilySearch/GEDCOM.io "$dest/familysearch"
  git -C "$dest/familysearch" sparse-checkout set testfiles
fi
echo "corpus ready in $dest"
