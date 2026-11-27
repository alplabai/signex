#!/usr/bin/env bash
#
# Controls for scripts/check_core_closure.sh: each shape the gate claims to
# catch must fail it, and each shape it must leave alone must pass. Every case
# runs on a small synthetic tree: a DocumentState naming `Doc`, a roots file
# naming `Event`, and the core files the case adds.
#
# Usage: scripts/test_check_core_closure.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_core_closure.sh"
CORE_REL="crates/signex-app/src/app/core"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Rerun all controls once per awk at hand (BSD awk, plus mawk and gawk).
if [[ -z "${CORE_CLOSURE_AWK:-}" ]]; then
  status=0 ran=0
  for awk_bin in /usr/bin/awk mawk gawk; do
    command -v "$awk_bin" >/dev/null 2>&1 || continue
    echo "== $awk_bin"
    ran=$((ran + 1))
    CORE_CLOSURE_AWK="$awk_bin" bash "$0" || status=1
  done
  if (( ran == 0 )); then
    echo "FAIL  no awk found (looked for /usr/bin/awk, mawk, gawk)"
    exit 1
  fi
  exit "$status"
fi

failures=0

BASE_CORE='pub struct Doc { pub inner: Inner }
pub struct Inner;
pub enum Event { Changed(Payload) }
pub struct Payload;'
BASE_ROOTS='Event  # what an edit reports back'

# run_case <name> <pass|fail> <extra core code> [message] [roots file]
#          [baseline lines] [extra file below app/core/] [its body]
# A fail case must print `message`; a pass case the OK line or `message`.
run_case() {
  local name="$1" expect="$2" want="${4:-}" t="$TMP/$1" status=0 out
  mkdir -p "$t/scripts" "$t/$CORE_REL" "$t/crates/signex-app/src/app/state"
  printf '%s\n' 'pub struct DocumentState {' '    /// not Unseen' \
    '    pub doc: crate::app::core::a::Doc,' '    pub n: u64,' '}' \
    > "$t/crates/signex-app/src/app/state/mod.rs"
  printf '%s\n' "$BASE_CORE" "$3" > "$t/$CORE_REL/a.rs"
  printf '%s\n' 'pub(crate) mod a;' > "$t/$CORE_REL.rs"
  printf '%s\n' '# roots' "${5:-$BASE_ROOTS}" > "$t/scripts/core-closure-roots.txt"
  printf '%s\n' '# baseline' "${6:-}" > "$t/scripts/core-closure-baseline.txt"
  [[ -n "${7:-}" ]] && printf '%s\n' "$8" > "$t/$CORE_REL/$7"
  out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" 2>&1)" || status=$?
  [[ "$expect" == pass && -z "$want" ]] && want="Core closure: OK"
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status: $out"
    failures=$((failures + 1))
  elif [[ "$out" != *"$want"* ]]; then
    echo "FAIL  $name — output lacks '$want': $out"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

A="$CORE_REL/a.rs"

# --- negative controls: the gate must pass these --------------------------

# Doc is named in DocumentState, Inner reached through Doc, Event a root.
run_case base_tree pass '' 'OK — 4 core types, 0 unreachable on the baseline'
run_case reachable_via_core_type_only pass 'pub struct Leaf;
pub struct Mid { pub leaf: Option<Leaf> }
pub enum Kind { A(Mid) }
impl Inner { pub fn kind(&self) -> Kind { Kind::A(Mid { leaf: None }) } }'
run_case reachable_via_alias_and_generics pass 'pub type Table = std::collections::HashMap<Key, Vec<Val>>;
pub struct Key; pub struct Val;
impl Payload { pub fn table(&self) -> Table { Table::new() } }'
run_case test_only_types_ignored pass '#[cfg(test)]
mod tests {
    pub struct Probe;
}
#[cfg(test)]
pub struct Fixture;' '' '' '' a_tests.rs 'pub struct Helper;'
run_case baselined_violation_passes pass 'pub struct ViewState { pub zoom: f32 }' '' '' \
  "$A ViewState"

# --- positive controls: the gate must flag these ---------------------------

run_case unreachable_view_type fail 'pub struct ViewState { pub zoom: f32 }' \
  "    $A ViewState"
run_case unreachable_says_move_out fail 'pub enum Tool { Select }' \
  "move the type out of app/core/ to its adapter module"
run_case doc_comment_is_no_edge fail '/// Shown by ViewState.
pub struct Shown;
pub struct ViewState;
impl Inner {
    /// Returns a ViewState.
    pub fn f(&self) {}
}' "    $A ViewState"
run_case string_is_no_edge fail 'pub struct ViewState;
impl Inner { pub fn name(&self) -> &str { "ViewState" } }' "    $A ViewState"
run_case fn_body_is_no_edge fail 'pub struct ViewState;
impl Inner { pub fn f(&self) -> u8 { let _v = ViewState; 1 } }' "    $A ViewState"
run_case test_fn_signature_is_no_edge fail 'pub struct ViewState;
impl Inner {
    #[cfg(test)]
    pub fn probe(&self) -> ViewState { ViewState }
}' "    $A ViewState"
run_case document_state_comment_is_no_edge fail 'pub struct Unseen;' "    $A Unseen"
run_case view_type_naming_core_is_still_unreachable fail 'pub struct ViewState { pub doc: Doc }' \
  "    $A ViewState"
run_case stale_baseline_entry fail '' "lists types that are no longer unreachable:
    $A Gone" '' "$A Gone"
run_case stale_says_delete fail '' "delete these lines from scripts/core-closure-baseline.txt" \
  '' "$A Inner"
run_case root_without_why fail 'pub struct Port;' "core-closure-roots.txt:3: Port" "$BASE_ROOTS
Port"
run_case root_with_empty_why fail 'pub struct Port;' "core-closure-roots.txt:3: Port  #" "$BASE_ROOTS
Port  #"
run_case root_names_no_core_type fail '' "names Missing, which is no type in app/core/" "$BASE_ROOTS
Missing  # gone"

# --- --generate writes a baseline the check then accepts -------------------

if CORE_CLOSURE_ROOT="$TMP/unreachable_view_type" bash "$GATE" --generate >/dev/null &&
  CORE_CLOSURE_ROOT="$TMP/unreachable_view_type" bash "$GATE" >/dev/null 2>&1; then
  echo "ok    generate_then_check"
else
  echo "FAIL  generate_then_check — the generated baseline does not pass"
  failures=$((failures + 1))
fi

# --- and the real tree passes ----------------------------------------------

if bash "$GATE" >/dev/null 2>&1; then
  echo "ok    real_tree"
else
  echo "FAIL  real_tree — the gate rejects the current workspace"
  failures=$((failures + 1))
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All core-closure controls passed."
