#!/usr/bin/env bash
#
# Release artifact smoke test (GH #342): run an INSTALLED signex binary and
# assert `--version` exits 0 and prints exactly `signex <version>` (the text
# from startup_args::version_text), so a broken install, a binary that dies at
# startup, or a tag/binary version mismatch fails the release leg.
#
#   scripts/smoke_release_artifact.sh <version> <cmd> [cmd-args...]
#
# <version> is the tag without its `v` (steps.version.outputs.version). The
# command is run as `<cmd> [cmd-args...] --version`, so wrappers work, e.g.
# `Signex.AppImage --appimage-extract-and-run`.
set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <version> <cmd> [cmd-args...]" >&2
    exit 2
fi
VERSION="$1"
shift

set +e
OUT="$("$@" --version 2>&1)"
STATUS=$?
set -e
OUT="${OUT%$'\r'}" # Windows console build prints CRLF

EXPECTED="signex ${VERSION}"
echo "smoke: $* --version -> exit ${STATUS}, output '${OUT}' (expected '${EXPECTED}')"
if [[ $STATUS -ne 0 || "$OUT" != "$EXPECTED" ]]; then
    echo "smoke: FAILED for $*" >&2
    exit 1
fi
