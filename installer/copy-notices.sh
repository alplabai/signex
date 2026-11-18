#!/bin/bash
# Copy Signex's licence and notice files into a release package (#2487).
# Bundled Apache-2.0 crates (solidcore) require their NOTICE text to travel
# with every distribution (Apache-2.0 §4(d)); THIRD_PARTY_NOTICES.md carries
# it, and NOTICE points at it.
#
# Invocation:
#   installer/copy-notices.sh <dest_dir>

set -euo pipefail

DEST="${1:?usage: copy-notices.sh <dest_dir>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

mkdir -p "$DEST"
for f in LICENSE LICENSE-APACHE NOTICE THIRD_PARTY_NOTICES.md; do
    if [[ ! -f "$ROOT/$f" ]]; then
        echo "missing $ROOT/$f" >&2
        exit 1
    fi
    cp "$ROOT/$f" "$DEST/"
done
