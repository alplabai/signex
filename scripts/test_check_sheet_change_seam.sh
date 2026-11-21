#!/usr/bin/env bash
#
# Controls for scripts/check_sheet_change_seam.sh (#2326 M5/M6, #2704): each
# shape the guard must catch fails it, and each shape it must leave alone
# passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_sheet_change_seam.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0
APP="crates/signex-app/src/app"

CORE_OK='pub(crate) fn settle_sheet_at(&mut self) -> Vec<u8> { Vec::new() }
pub(super) fn warning() {}
fn private_step() {}'
LIST_OK='# the exception
handlers/editing_commands/sheet_on.rs settle_and_project'
WRAPPER='impl Signex {
    fn settle_and_project(&mut self) { let _ = self.document_state.settle_sheet_at(); }
}'
CALL='fn f(app: &mut Signex) { let _ = app.document_state.settle_sheet_at(); }'

# check <name> <expected: fail|pass> <tree>
check() {
  local status=0
  SHEET_CHANGE_SEAM_ROOT="$3" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$2" == fail && $status -ne 1 ]] || [[ "$2" == pass && $status -ne 0 ]]; then
    echo "FAIL  $1 — expected $2, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $1"
  fi
}

# run_case <name> <expected> <core mod.rs> <allowlist> <sheet_on.rs> [<file under app> <body>]
run_case() {
  local name="$1" expect="$2" tree="$TMP/$1"
  mkdir -p "$tree/$APP/state/sheet_change" "$tree/$APP/handlers/editing_commands" "$tree/scripts"
  printf '%s\n' "$3" > "$tree/$APP/state/sheet_change/mod.rs"
  printf '%s\n' "$4" > "$tree/scripts/sheet-change-wrappers.txt"
  printf '%s\n' "$5" > "$tree/$APP/handlers/editing_commands/sheet_on.rs"
  if [[ $# -ge 7 ]]; then
    mkdir -p "$tree/$APP/$(dirname "$6")"
    printf '%s\n' "$7" > "$tree/$APP/$6"
  fi
  check "$name" "$expect" "$tree"
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
# Shrink-only: a line beyond the named exception fails, even with a caller.
run_case list_grows fail "$CORE_OK" "$LIST_OK
handlers/net_colors.rs f" "$WRAPPER" handlers/net_colors.rs "$CALL"
run_case stale_line fail "$CORE_OK" "$LIST_OK
mutation_gateway.rs finish_schematic_mutation" "$WRAPPER"
run_case same_suffix pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f(v: &mut View) { v.resettle_sheet_at(); }'
run_case super_fn_name pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/net_colors.rs 'fn f() { warning(); private_step(); }'
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
run_case core_comment_only pass "// ui_state, panel_ctx and crate::diagnostics are named in prose only.
$CORE_OK" "$LIST_OK" "$WRAPPER"

# A tree without the core module or the list is an error, never a pass.
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
