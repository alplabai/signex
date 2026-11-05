#!/usr/bin/env bash
#
# Native-side-effect gate (#1931): only the native adapter in
# crates/signex-app/src/io_ports/native.rs may open an OS file dialog
# (`rfd::`), a browser (`webbrowser::`) or a launcher process (`open`,
# `xdg-open`, `explorer`). Everything else goes through the picker and opener
# ports, which a headless run answers from plain arguments or refuses.
#
# Scope: production and test code under crates/*/src. Comment-only lines are
# ignored, so a doc comment may still name the crates.
#
# Usage: scripts/check_io_ports.sh   (IO_PORTS_ROOT overrides the tree, for
# scripts/test_check_io_ports.sh)
set -euo pipefail

ROOT="${IO_PORTS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ADAPTER="crates/signex-app/src/io_ports/native.rs"
PATTERN='\b(rfd|webbrowser)::|Command::new\("(open|xdg-open|explorer)"\)'

cd "$ROOT"
hits="$(grep -rnE --include='*.rs' "$PATTERN" crates/*/src 2>/dev/null \
  | grep -v "^$ADAPTER:" \
  | awk -F: '{ line = $0; sub(/^[^:]*:[^:]*:/, "", line); if (line !~ /^[[:space:]]*\/\//) print $0 }' || true)"

if [[ -n "$hits" ]]; then
  echo "Native-side-effect gate: FAIL — use crate::io_ports (Picker / Opener) instead:"
  echo "$hits"
  exit 1
fi
echo "Native-side-effect gate: OK — rfd, webbrowser and launchers appear only in $ADAPTER."
