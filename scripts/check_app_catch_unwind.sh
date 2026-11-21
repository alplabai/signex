#!/usr/bin/env bash
#
# catch_unwind ratchet (#2651, ADR-0006): a `catch_unwind` in the app hides a
# half-done state change behind a caught panic. The core keeps its invariants
# by doing nothing foreign between taking a value out and putting it back, so
# the production `catch_unwind` calls under crates/signex-app/src/app may only
# get fewer. The count must equal scripts/app-catch-unwind-baseline.txt: more
# fails, and fewer fails until the same change lowers the baseline.
#
# The calls come from ast-grep (scripts/app-catch-unwind.yml): `catch_unwind(…)`
# and `<path>::catch_unwind(…)`, not inside an item under `#[cfg(test)]`.
# Test files (`tests.rs`, `*_tests.rs`, anything under a `tests/` or
# `*_tests/` directory) are skipped. Blind spots: a renamed import (`use std::panic::catch_unwind as
# c`), `catch_unwind` passed as a value, and the FutureExt method
# `fut.catch_unwind()`, which catches a task's panic, not one inside `update`.
#
# Usage: scripts/check_app_catch_unwind.sh   (APP_CATCH_UNWIND_ROOT overrides
# the tree, for scripts/test_check_app_catch_unwind.sh)
set -euo pipefail

ROOT="${APP_CATCH_UNWIND_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
RULE="$(cd "$(dirname "$0")" && pwd)/app-catch-unwind.yml"
SRC="crates/signex-app/src/app"
BASELINE="scripts/app-catch-unwind-baseline.txt"

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "catch_unwind ratchet: FAIL — $tool is not installed."
    exit 2
  fi
done
if [[ ! -d "$SRC" || ! -f "$BASELINE" ]]; then
  echo "catch_unwind ratchet: FAIL — no $SRC or $BASELINE under $ROOT."
  exit 2
fi

if ! scan="$(ast-grep scan --rule "$RULE" --json=compact "$SRC")"; then
  echo "catch_unwind ratchet: FAIL — ast-grep could not run $RULE."
  exit 2
fi
hits="$(printf '%s' "$scan" | jq -r '.[]
  | select(.file | test("(^|/|_)tests/|(^|/|_)tests\\.rs$") | not)
  | "\(.file):\(.range.start.line + 1)"' | sort)"
found="$(printf '%s\n' "$hits" | grep -c . || true)"
allowed="$(grep -vE '^[[:space:]]*(#|$)' "$BASELINE" | head -n 1 | tr -d '[:space:]')"

if [[ ! "$allowed" =~ ^[0-9]+$ ]]; then
  echo "catch_unwind ratchet: FAIL — $BASELINE holds no count."
  exit 2
fi
if (( found > allowed )); then
  echo "catch_unwind ratchet: FAIL — $found production catch_unwind calls under $SRC, baseline $allowed."
  echo "Do the work so a panic leaves no value taken out, instead of catching it:"
  printf '%s\n' "$hits" | awk 'NF { print "  " $0 }'
  exit 1
fi
if (( found < allowed )); then
  echo "catch_unwind ratchet: FAIL — $found production catch_unwind calls, baseline $allowed; lower $BASELINE to $found."
  exit 1
fi
echo "catch_unwind ratchet: OK — $found production catch_unwind calls under $SRC (baseline $allowed)."
