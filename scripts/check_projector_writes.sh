#!/usr/bin/env bash
#
# Projector write guard (#2326 slice 3, ADR-0006): crates/signex-app/src/app/
# projector/ only draws documents into the GUI. It writes no engine-map entry
# (`insert_engine`, `remove_engine`, `clear_active_engine`, `engines.insert`,
# `engines.remove`), no `active_path` and no dirty or saved mark; those belong
# to the core half that owns the documents.
#
# The writes come from ast-grep (scripts/projector-writes.yml), not inside an
# item under `#[cfg(test)]`. Test files (`tests.rs`, `*_tests.rs`, anything
# under a `tests/` or `*_tests/` directory) are skipped. Blind spots: a renamed
# import or a method passed as a value (`.map(Engine::mark_saved)`), a write
# through a local alias of the map (`let e = &mut ..engines; e.insert(..)`),
# and a write hidden in a helper defined outside the projector.
#
# Usage: scripts/check_projector_writes.sh   (PROJECTOR_WRITES_ROOT overrides
# the tree, for scripts/test_check_projector_writes.sh)
set -euo pipefail

ROOT="${PROJECTOR_WRITES_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
RULE="$(cd "$(dirname "$0")" && pwd)/projector-writes.yml"
SRC="crates/signex-app/src/app/projector"

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Projector write guard: FAIL — $tool is not installed."
    exit 2
  fi
done
if [[ ! -d "$SRC" ]]; then
  echo "Projector write guard: FAIL — no $SRC under $ROOT."
  exit 2
fi

if ! scan="$(ast-grep scan --rule "$RULE" --json=compact "$SRC")"; then
  echo "Projector write guard: FAIL — ast-grep could not run $RULE."
  exit 2
fi
hits="$(printf '%s' "$scan" | jq -r '.[]
  | select(.file | test("(^|/|_)tests/|(^|/|_)tests\\.rs$") | not)
  | "\(.file):\(.range.start.line + 1): \(.text | split("\n")[0])"' | sort)"

if [[ -n "$hits" ]]; then
  echo "Projector write guard: FAIL — $SRC writes document state:"
  printf '%s\n' "$hits" | awk '{ print "  " $0 }'
  echo "  Do the write in the core half (app/state) and project its events here (#2326)."
  exit 1
fi
echo "Projector write guard: OK — no engine-map, active_path or dirty/saved writes in $SRC."
