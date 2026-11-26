#!/bin/bash
# Copy the license text of every bundled font into a release package (#1179).
# The fonts are compiled into the binary, so their licenses must travel with
# it: each `OFL-*.md` next to the app's font files, and each licence of the
# typst-assets fonts the Design Notebook embeds (`crates/signex-notebook/fonts`,
# #3023), goes to <dest>.
#
# Invocation:
#   installer/copy-font-licenses.sh <dest_dir>

set -euo pipefail

DEST="${1:?usage: copy-font-licenses.sh <dest_dir>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FONTS="$ROOT/crates/signex-app/assets/fonts"
NOTEBOOK_FONTS="$ROOT/crates/signex-notebook/fonts"

mkdir -p "$DEST"
shopt -s nullglob
licenses=("$FONTS"/OFL-*.md)
if [[ ${#licenses[@]} -eq 0 ]]; then
    echo "no font licenses found in $FONTS" >&2
    exit 1
fi
notebook=("$NOTEBOOK_FONTS"/*.md)
if [[ ${#notebook[@]} -eq 0 ]]; then
    echo "no font licenses found in $NOTEBOOK_FONTS" >&2
    exit 1
fi
cp "${licenses[@]}" "${notebook[@]}" "$DEST/"
