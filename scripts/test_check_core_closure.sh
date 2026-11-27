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
APP="crates/signex-app/src/app"
CORE_REL="$APP/core"
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

# commit <tree>: the tree as one commit, for --no-growth.
commit() {
  git -C "$1" init -q
  git -C "$1" add -A
  git -C "$1" -c user.name=c -c user.email=c@c commit -q -m base
}

# judge <name> <pass|fail> <message> <status> <output>
judge() {
  local want="$3"
  [[ "$2" == pass && -z "$want" ]] && want="Core closure: OK"
  if [[ "$2" == fail && $4 -ne 1 ]] || [[ "$2" == pass && $4 -ne 0 ]]; then
    echo "FAIL  $1 — expected $2, gate exit $4: $5"
    failures=$((failures + 1))
  elif [[ "$5" != *"$want"* ]]; then
    echo "FAIL  $1 — output lacks '$want': $5"
    failures=$((failures + 1))
  else
    echo "ok    $1"
  fi
}

# run_case <name> <pass|fail> <extra code of core/a.rs> [message] [roots file]
#          [baseline lines]
# Optional, set on the call: CORE_RS (extra code of app/core.rs); DOC_FIELDS
# (extra fields of DocumentState); XF1/XB1 and
# XF2/XB2 (another file below the tree root and its body); GROW_BASE (commit
# the tree with this baseline, then check with --no-growth HEAD).
# A fail case must print `message`; a pass case the OK line or `message`.
run_case() {
  local t="$TMP/$1" status=0 out args=()
  mkdir -p "$t/scripts" "$t/$CORE_REL" "$t/$APP/state"
  printf '%s\n' 'pub struct DocumentState {' '    /// not Unseen' \
    '    pub doc: crate::app::core::a::Doc,' '    pub n: u64,' "${DOC_FIELDS:-}" '}' \
    > "$t/$APP/state/mod.rs"
  printf '%s\n' "$BASE_CORE" "$3" > "$t/$CORE_REL/a.rs"
  printf '%s\n' 'pub(crate) mod a;' "${CORE_RS:-}" > "$t/$CORE_REL.rs"
  printf '%s\n' '# roots' "${5:-$BASE_ROOTS}" > "$t/scripts/core-closure-roots.txt"
  if [[ -n "${XF1:-}" ]]; then mkdir -p "$(dirname "$t/$XF1")"; printf '%s\n' "$XB1" > "$t/$XF1"; fi
  if [[ -n "${XF2:-}" ]]; then mkdir -p "$(dirname "$t/$XF2")"; printf '%s\n' "$XB2" > "$t/$XF2"; fi
  if [[ -n "${GROW_BASE:-}" ]]; then
    printf '%s\n' '# baseline' "$GROW_BASE" > "$t/scripts/core-closure-baseline.txt"
    commit "$t"
    args=(--no-growth HEAD)
  fi
  printf '%s\n' '# baseline' "${6:-}" > "$t/scripts/core-closure-baseline.txt"
  out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" ${args[@]+"${args[@]}"} 2>&1)" || status=$?
  judge "$1" "$2" "${4:-}" "$status" "$out"
}

A="$CORE_REL/a.rs"
T="$CORE_REL/a_tests.rs"
V='pub struct ViewState;'
AGENT='pub struct AgentEdits { pub joined: Vec<crate::app::core::a::Joined> }'

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
run_case struct_variant_field_type_is_an_edge pass 'pub struct Leaf;
pub enum Mode { Plain, At { leaf: Leaf } }
impl Inner { pub fn mode(&self) -> Mode { Mode::Plain } }'
run_case test_items_ignored pass '#[cfg(test)]
mod tests {
    pub struct Probe;
    macro_rules! probe { () => {}; }
}
#[cfg(test)]
pub struct Fixture;'
CORE_RS='#[cfg(test)]
mod a_tests;' XF1="$T" XB1='pub struct Helper;' \
  run_case test_mounted_file_ignored pass ''
CORE_RS='#[cfg(test)]
mod t;' XF1="$CORE_REL/t/mod.rs" XB1='mod deeper;' XF2="$CORE_REL/t/deeper.rs" XB2='pub struct Helper;' \
  run_case file_below_test_module_ignored pass ''
run_case baselined_violation_passes pass "$V" '' '' "$A ViewState"
run_case fn_body_and_const_macros_are_fine pass 'impl Inner { pub fn v(&self) -> Vec<u8> { vec![1] } }
const N: usize = m!();'
DOC_FIELDS='    pub agent: agent::AgentEdits,' XF1="$APP/state/agent.rs" XB1="$AGENT" \
  run_case root_via_state_type_of_a_field pass 'pub struct Joined;'
XF1="$APP/edit.rs" XB1='impl DocumentState {
    pub fn settle(&mut self) -> Vec<Reported> { Vec::new() }
}' run_case root_via_document_state_method pass 'pub struct Reported;'
GROW_BASE="$A ViewState
$A Gone" run_case baseline_shrinks_against_ref pass "$V" '' '' "$A ViewState"
GROW_BASE="$A ViewState" run_case baseline_same_as_ref pass "$V" '' '' "$A ViewState"

# --- positive controls: the gate must flag these ---------------------------

run_case unreachable_view_type fail "$V" "    $A ViewState"
run_case unreachable_says_move_out fail 'pub enum Tool { Select }' \
  "move the type out of app/core/ to its adapter module"
run_case doc_comment_is_no_edge fail '/// Shown by ViewState.
pub struct Shown;
pub struct ViewState;
impl Inner {
    /// Returns a ViewState.
    pub fn f(&self) {}
}' "    $A ViewState"
run_case string_is_no_edge fail "$V"'
impl Inner { pub fn name(&self) -> &str { "ViewState" } }' "    $A ViewState"
run_case fn_body_is_no_edge fail "$V"'
impl Inner { pub fn f(&self) -> u8 { let _v = ViewState; 1 } }' "    $A ViewState"
run_case test_fn_signature_is_no_edge fail "$V"'
impl Inner {
    #[cfg(test)]
    pub fn probe(&self) -> ViewState { ViewState }
}' "    $A ViewState"
run_case document_state_comment_is_no_edge fail 'pub struct Unseen;' "    $A Unseen"
run_case view_type_naming_core_is_still_unreachable fail 'pub struct ViewState { pub doc: Doc }' \
  "    $A ViewState"
run_case variant_name_is_no_edge fail "$V"'
pub enum Mode { Plain, #[allow(dead_code)] ViewState = 2, Held(Payload) }
impl Inner { pub fn mode(&self) -> Mode { Mode::Plain } }' "    $A ViewState"
XF1="$APP/state/agent.rs" XB1="$AGENT" \
  run_case state_type_not_named_is_no_root fail 'pub struct Joined;' "    $A Joined" \
  "$BASE_ROOTS" ''
XF1="$APP/edit.rs" XB1='impl DocumentState {
    #[cfg(test)]
    pub fn probe(&self) -> Reported { Reported }
}' run_case test_document_state_method_is_no_root fail 'pub struct Reported;' "    $A Reported"
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
XF1="$T" XB1='pub struct Helper;' \
  run_case unmounted_tests_file_is_scanned fail '' "    $T Helper"
CORE_RS='mod a_tests;' XF1="$T" XB1='pub struct Helper;' \
  run_case plain_mounted_tests_file_is_scanned fail '' "    $T Helper"
XF1="$CORE_REL/b.rs" XB1='pub struct Inner;' \
  run_case duplicate_name fail '' "two app/core/ types are named Inner"
run_case shadowed_std_name fail 'pub struct Uuid;' "$A Uuid is named like a std or dependency type"
# shellcheck disable=SC2016  # `$n` is Rust macro syntax, not shell
run_case macro_rules_in_core fail 'macro_rules! mk { ($n:ident) => { pub struct $n; }; }' \
  "$A:5 defines or calls a macro at item level"
run_case item_macro_call_in_core fail 'mk!(Hidden);' "$A:5 defines or calls a macro at item level"
run_case item_macro_in_inline_mod fail 'mod inner { bitflags::bitflags! { pub struct F: u8 {} } }' \
  "defines or calls a macro at item level"
GROW_BASE="$A Other" run_case baseline_grew_against_ref fail "$V"'
pub struct Other;' "core-closure-baseline.txt grew against HEAD:
    $A ViewState" '' "$A Other
$A ViewState"

# --- --no-growth on a hand-made history -------------------------------------

# --generate takes a new type into the baseline; --no-growth still fails it.
t="$TMP/unreachable_view_type"
commit "$t"
status=0
out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" --generate 2>&1)" || status=$?
judge generate_writes_baseline pass 'unreachable types' "$status" "$out"
status=0
out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" 2>&1)" || status=$?
judge generate_then_check pass '' "$status" "$out"
status=0
out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" --no-growth HEAD 2>&1)" || status=$?
judge generate_then_no_growth fail "grew against HEAD:
    $A ViewState" "$status" "$out"
status=0
out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" --no-growth no-such-ref 2>&1)" || status=$?
judge no_growth_bad_ref fail "no-such-ref is no commit here" "$status" "$out"
# A base without the baseline file (the commit that adds the guard) passes.
t="$TMP/base_tree"
commit "$t"
git -C "$t" rm -q --cached scripts/core-closure-baseline.txt
git -C "$t" -c user.name=c -c user.email=c@c commit -q -m drop
status=0
out="$(CORE_CLOSURE_ROOT="$t" bash "$GATE" --no-growth HEAD 2>&1)" || status=$?
judge no_growth_base_without_baseline pass "has no scripts/core-closure-baseline.txt" "$status" "$out"

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
