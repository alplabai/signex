#!/bin/bash
# Copy the license text of every bundled font into a release package (#1179).
# The fonts are compiled into the binary, so their licenses must travel with
# it: each `OFL-*.md` next to the font files goes to <dest>.
#
# Invocation:
#   installer/copy-font-licenses.sh <dest_dir>

set -euo pipefail

DEST="${1:?usage: copy-font-licenses.sh <dest_dir>}"
FONTS="$(cd "$(dirname "$0")/.." && pwd)/crates/signex-app/assets/fonts"

mkdir -p "$DEST"
shopt -s nullglob
licenses=("$FONTS"/OFL-*.md)
if [[ ${#licenses[@]} -eq 0 ]]; then
    echo "no font licenses found in $FONTS" >&2
    exit 1
fi
cp "${licenses[@]}" "$DEST/"
