#!/usr/bin/env bash
#
# Controls for scripts/check_sheet_change_seam.sh (#2326 M5/M6, issue #2704):
# each shape the guard must catch fails it, and each shape it must leave alone
# passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_sheet_change_seam.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0
APP="crates/signex-app/src/app"

CORE_OK='pub(crate) fn settle_sheet_at(&mut self) -> Vec<u8> { Vec::new() }
pub(super) fn warning_notice() {}
pub(self) fn self_only() {}
fn private_step() {}'
LIST_OK='# the exception
handlers/editing_commands/sheet_on.rs after_sheet_change'
WRAPPER='impl Signex {
    fn after_sheet_change(&mut self) { let _ = self.document_state.settle_sheet_at(); }
}'
CALL='fn f(app: &mut Signex) { let _ = app.document_state.settle_sheet_at(); }'

# run_case <name> <expected: fail|pass> <core mod.rs> <allowlist> <sheet_on.rs> [<file under app> <body>]
# The trees allow one wrapper, the one LIST_OK names (its `cksum` below).
run_case() {
  local name="$1" expect="$2" tree="$TMP/$1" status=0
  mkdir -p "$tree/$APP/state/sheet_change" "$tree/$APP/handlers/editing_commands" "$tree/scripts"
  printf '%s\n' "$3" > "$tree/$APP/state/sheet_change/mod.rs"
  printf '%s\n' "$4" > "$tree/scripts/sheet-change-wrappers.txt"
  printf '%s\n' "$5" > "$tree/$APP/handlers/editing_commands/sheet_on.rs"
  if [[ $# -ge 7 ]]; then
    mkdir -p "$tree/$APP/$(dirname "$6")"
    printf '%s\n' "$7" > "$tree/$APP/$6"
  fi
  SHEET_CHANGE_SEAM_MAX=1 SHEET_CHANGE_SEAM_CKSUM=2413533166 SHEET_CHANGE_SEAM_ROOT="$tree" \
    bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

run_case listed_wrapper pass "$CORE_OK" "$LIST_OK" "$WRAPPER"
# Positive control: one extra caller outside the wrappers goes red.
run_case extra_caller fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs "$CALL"
run_case other_fn_in_wrapper_file fail "$CORE_OK" "$LIST_OK" "$WRAPPER
$CALL"
run_case multi_line fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(app: &mut Signex) {
    let _ = app
        .document_state
        .settle_sheet_at();
}'
run_case by_path fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f() { sheet_change::settle_sheet_at(); }'
# Calls inside macro token trees.
run_case in_vec_macro fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/dock/sheet_variables.rs 'impl Signex {
    fn f(&mut self, p: PathBuf) {
        let _e = vec![self.document_state.settle_sheet_at(&p, DocumentPatch::NONE, ChangeCause::Edit)];
    }
}'
run_case in_assert_macro fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(s: &mut DocumentState) { assert!(s.settle_sheet_at().is_empty()); }'
run_case macro_in_wrapper pass "$CORE_OK" "$LIST_OK" 'impl Signex {
    fn after_sheet_change(&mut self) { let _ = vec![self.document_state.settle_sheet_at()]; }
}'
# A core function named as a value, or imported, counts as a use.
run_case path_as_value fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(v: Vec<DocumentState>) { let _ = v.into_iter().map(DocumentState::settle_sheet_at); }'
run_case bare_value fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f() { let g = settle_sheet_at; }'
run_case import fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'use crate::app::state::sheet_change::settle_sheet_at;'
run_case field_of_that_name pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(s: &S) -> u8 { s.settle_sheet_at }'
# No trait and no trait impl in the core module.
run_case trait_in_core fail "$CORE_OK
pub(crate) trait Reach { fn reach(&mut self) { let _ = self.settle_sheet_at(); } }" "$LIST_OK" "$WRAPPER"
run_case trait_impl_in_core fail "$CORE_OK
impl Reach for DocumentState { fn reach(&mut self) {} }" "$LIST_OK" "$WRAPPER"
run_case generic_trait_impl_in_core fail "$CORE_OK
impl From<u8> for SheetEdit { fn from(_: u8) -> Self { todo!() } }" "$LIST_OK" "$WRAPPER"
run_case inherent_impl_in_core pass "$CORE_OK
impl DocumentState { fn helper(&self) {} }" "$LIST_OK" "$WRAPPER"
# Any pub(...) but pub(self) counts as visible.
run_case pub_in_path fail "$CORE_OK
pub(in crate::app) fn probe_hidden() {}" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f() { probe_hidden(); }'
run_case pub_super fail "$CORE_OK" "$LIST_OK" "$WRAPPER" state/other.rs 'fn f() { warning_notice(); }'
run_case pub_self pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f() { self_only(); private_step(); }'
# The name set is pinned: swapping the one wrapper for another fails, even
# with the count unchanged and the call inside the new wrapper.
run_case list_swaps_a_name fail "$CORE_OK" 'handlers/editing_commands/sheet_on.rs other_fn' 'impl Signex {
    fn other_fn(&mut self) { let _ = self.document_state.settle_sheet_at(); }
}'
# Shrink-only: a line beyond MAX_WRAPPERS fails, even with a caller behind it.
run_case list_grows fail "$CORE_OK" "$LIST_OK
handlers/net_colors.rs f" "$WRAPPER" handlers/net_colors.rs "$CALL"
run_case list_below_max fail "$CORE_OK" '# empty' 'fn g() {}'
run_case stale_line fail "$CORE_OK" 'mutation_gateway.rs finish_schematic_mutation' "$WRAPPER"
run_case same_suffix pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(v: &mut View) { v.resettle_sheet_at(); }'
run_case tests_file pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors_tests.rs "$CALL"
run_case tests_dir pass "$CORE_OK" "$LIST_OK" "$WRAPPER" edit_tail_seam_tests/dirty.rs "$CALL"
run_case cfg_test_mod pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs "fn g() {}
#[cfg(test)]
mod tests { $CALL }"
run_case core_calls_itself pass "$CORE_OK
fn inner(s: &mut DocumentState) { let _ = s.settle_sheet_at(); }" "$LIST_OK" "$WRAPPER"
run_case core_logs fail "$CORE_OK
fn log() { crate::diagnostics::log_info(\"x\"); }" "$LIST_OK" "$WRAPPER"
run_case core_uses_diagnostics fail "use crate::diagnostics;
$CORE_OK" "$LIST_OK" "$WRAPPER"
run_case core_reads_ui_state fail "$CORE_OK
fn f(app: &Signex) { let _ = &app.ui_state; }" "$LIST_OK" "$WRAPPER"
run_case core_reads_panel_ctx fail "$CORE_OK
fn f(s: &DocumentState) { let _ = &s.panel_ctx; }" "$LIST_OK" "$WRAPPER"
run_case core_reads_window_panel_ctx fail "$CORE_OK
fn f(s: &Signex) { let _ = s.window_panel_ctx(); }" "$LIST_OK" "$WRAPPER"
run_case core_reads_interaction fail "$CORE_OK
fn f(s: &Signex) { let _ = &s.interaction_state; }" "$LIST_OK" "$WRAPPER"
run_case core_logs_through_projector fail "$CORE_OK
fn f(n: &Notice) { crate::app::projector::log_notice(n); }" "$LIST_OK" "$WRAPPER"
run_case core_tracing fail "$CORE_OK
fn f() { tracing::warn!(\"x\"); }" "$LIST_OK" "$WRAPPER"
run_case core_comment_only pass "// ui_state, panel_ctx and crate::diagnostics are named in prose only.
$CORE_OK" "$LIST_OK" "$WRAPPER"

# A tree without the core module is an error, never a pass.
mkdir -p "$TMP/no_core/$APP" "$TMP/no_core/scripts"
printf '%s\n' "$LIST_OK" > "$TMP/no_core/scripts/sheet-change-wrappers.txt"
status=0
SHEET_CHANGE_SEAM_ROOT="$TMP/no_core" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_core — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_core"
fi

if [[ $failures -ne 0 ]]; then
  echo "check_sheet_change_seam controls: $failures case(s) failed"
  exit 1
fi
echo "check_sheet_change_seam controls: OK"
