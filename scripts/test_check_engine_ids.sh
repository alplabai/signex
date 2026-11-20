#!/usr/bin/env bash
#
# Controls for scripts/check_engine_ids.sh: each shape the gate must catch
# fails it, and each shape it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_engine_ids.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

# run_case <name> <expected: fail|pass> <file under engine src> <body> [allowlist]
run_case() {
  local name="$1" expect="$2" file="$3" body="$4" allow="${5:-}" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/crates/signex-engine/src/$(dirname "$file")" "$tree/scripts"
  printf '%s\n' "$body" > "$tree/crates/signex-engine/src/$file"
  printf '%s\n' "$allow" > "$tree/scripts/engine-ids-allowlist.txt"
  status=0
  ENGINE_IDS_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

run_case now_v7 fail exec/place.rs 'fn place() { let id = Uuid::now_v7(); }'
run_case new_v4 fail lib.rs 'fn make() { let id = uuid::Uuid::new_v4(); }'
run_case fn_pointer fail lib.rs 'fn make() { x.unwrap_or_else(Uuid::now_v7) }'
run_case other_fn_same_file fail sheet.rs 'fn listed() { Uuid::now_v7(); }
pub(crate) fn other() { Uuid::now_v7(); }' 'sheet.rs:listed'
run_case stale_entry fail sheet.rs 'fn listed() {}' 'sheet.rs:listed'
run_case allowlisted pass transform/autoplace.rs 'fn junction_at() {
    let id = Uuid::now_v7();
}' 'transform/autoplace.rs:junction_at'
run_case comment pass lib.rs '// Never call Uuid::now_v7 here.
fn f() {}'
run_case trailing_test_mod pass lib.rs 'fn f() {}
#[cfg(test)]
mod tests {
    fn t() { Uuid::now_v7(); }
}'
run_case test_file pass place_tests.rs 'fn t() { Uuid::now_v7(); }'
run_case clean pass lib.rs 'fn f(ids: &mut dyn IdSource) { ids.next_id(); }'
# A listed helper mints for whoever calls it: a new caller grows its count.
run_case helper_new_caller fail transform/autoplace.rs 'fn junction_at() { Uuid::now_v7(); }
fn other() { junction_at(); }' 'transform/autoplace.rs:junction_at'
run_case helper_counted_caller pass transform/autoplace.rs 'fn junction_at() { Uuid::now_v7(); }
fn other() { junction_at(); }' 'transform/autoplace.rs:junction_at 1'
run_case helper_fn_pointer fail transform/autoplace.rs 'fn junction_at() { Uuid::now_v7(); }
fn other() { xs.map(super::autoplace::junction_at) }' 'transform/autoplace.rs:junction_at'
run_case helper_count_stale fail transform/autoplace.rs 'fn junction_at() { Uuid::now_v7(); }' 'transform/autoplace.rs:junction_at 1'
run_case helper_caller_in_test pass transform/autoplace.rs 'fn junction_at() { Uuid::now_v7(); }
#[cfg(test)]
mod tests {
    fn t() { junction_at(); }
}' 'transform/autoplace.rs:junction_at'

# A tree without the engine crate is an error, never a pass.
mkdir -p "$TMP/no_engine"
status=0
ENGINE_IDS_ROOT="$TMP/no_engine" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_engine — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_engine"
fi

if [[ $failures -ne 0 ]]; then
  echo "check_engine_ids controls: $failures case(s) failed"
  exit 1
fi
echo "check_engine_ids controls: OK"
