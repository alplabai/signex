#!/usr/bin/env bash
#
# Controls for scripts/check_legacy_english.sh: each shape the gate claims to
# catch must fail it, and each shape it must leave alone must pass. A gate
# that is never shown a violation can go dead and still say "OK".
#
# Usage: scripts/test_check_legacy_english.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_legacy_english.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

# put <tree> <relative path> <body>
put() {
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
}

# new_tree <name> — one call of each constructor and a baseline of 1 each.
new_tree() {
  local tree="$TMP/$1"
  mkdir -p "$tree/scripts"
  put "$tree" scripts/legacy-english-baseline.txt 'command_outcome_disabled=1
translatable_legacy_english=1'
  put "$tree" crates/a/src/lib.rs 'fn a() { let _ = CommandOutcome::disabled("no"); }
fn b() { let _ = Translatable::legacy_english("no"); }'
  echo "$tree"
}

# expect <name> <fail|pass> <tree>
expect() {
  local name="$1" want="$2" tree="$3" status=0
  LEGACY_ENGLISH_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$want" == fail && $status -ne 1 ]] || [[ "$want" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $want, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

# --- positive controls: the gate must flag these -------------------------

t="$(new_tree a_new_disabled_call)"
put "$t" crates/b/src/lib.rs 'fn c() { let _ = CommandOutcome::disabled("new"); }'
expect a_new_disabled_call fail "$t"

t="$(new_tree a_qualified_legacy_call)"
put "$t" crates/b/src/lib.rs 'fn c() { let _ = signex_i18n_ids::Translatable::legacy_english("new"); }'
expect a_qualified_legacy_call fail "$t"

t="$(new_tree a_paid_debt_not_locked_in)"
put "$t" crates/a/src/lib.rs 'fn b() { let _ = Translatable::legacy_english("no"); }'
expect a_paid_debt_not_locked_in fail "$t"

t="$(new_tree a_missing_baseline)"
rm "$t/scripts/legacy-english-baseline.txt"
expect a_missing_baseline fail "$t"

# --- negative controls: the gate must leave these alone ------------------

t="$(new_tree the_baseline_as_it_is)"
expect the_baseline_as_it_is pass "$t"

t="$(new_tree calls_in_test_files)"
put "$t" crates/b/src/x_tests.rs 'fn c() { let _ = CommandOutcome::disabled("t"); }'
put "$t" crates/b/src/tests_print.rs 'fn c() { let _ = CommandOutcome::disabled("t"); }'
put "$t" crates/b/tests/it.rs 'fn c() { let _ = Translatable::legacy_english("t"); }'
expect calls_in_test_files pass "$t"

t="$(new_tree a_comment_and_the_definition)"
put "$t" crates/b/src/lib.rs '// CommandOutcome::disabled("x")
impl Translatable { pub fn legacy_english(t: String) -> Self { todo!() } }'
expect a_comment_and_the_definition pass "$t"

if (( failures )); then
  echo "Legacy-English ratchet controls: $failures failed."
  exit 1
fi
echo "Legacy-English ratchet controls: OK."
