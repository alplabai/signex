#!/usr/bin/env bash
#
# Controls for scripts/check_sheet_change_seam.sh: each shape the guard must
# catch fails it, and each shape it must leave alone passes.
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

# run_case <name> <expected: fail|pass> <core mod.rs body> <file under app> <body>
run_case() {
  local name="$1" expect="$2" core="$3" file="$4" body="$5" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/$APP/state/sheet_change" "$tree/$APP/$(dirname "$file")"
  printf '%s\n' "$core" > "$tree/$APP/state/sheet_change/mod.rs"
  printf '%s\n' "$body" > "$tree/$APP/$file"
  status=0
  SHEET_CHANGE_SEAM_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

CALL='fn f(app: &mut Signex) { let _ = app.document_state.settle_sheet_at(); }'

run_case wrapper_gateway pass "$CORE_OK" mutation_gateway.rs "$CALL"
run_case wrapper_sheet_on pass "$CORE_OK" handlers/editing_commands/sheet_on.rs "$CALL"
run_case wrapper_read_only pass "$CORE_OK" read_only_refusal.rs "$CALL"
run_case other_file fail "$CORE_OK" handlers/net_colors.rs "$CALL"
run_case multi_line fail "$CORE_OK" handlers/net_colors.rs 'fn f(app: &mut Signex) {
    let _ = app
        .document_state
        .settle_sheet_at();
}'
run_case by_path fail "$CORE_OK" handlers/net_colors.rs 'fn f() { sheet_change::settle_sheet_at(); }'
run_case same_suffix pass "$CORE_OK" handlers/net_colors.rs 'fn f(v: &mut View) { v.resettle_sheet_at(); }'
run_case super_fn_name pass "$CORE_OK" handlers/net_colors.rs 'fn f() { warning(); private_step(); }'
run_case tests_file pass "$CORE_OK" handlers/net_colors_tests.rs "$CALL"
run_case tests_dir pass "$CORE_OK" edit_tail_seam_tests/dirty.rs "$CALL"
run_case cfg_test_mod pass "$CORE_OK" handlers/net_colors.rs "fn g() {}
#[cfg(test)]
mod tests { $CALL }"
run_case core_calls_itself pass "$CORE_OK
fn inner(s: &mut DocumentState) { let _ = s.settle_sheet_at(); }" handlers/x.rs 'fn f() {}'
run_case core_logs fail "$CORE_OK
fn log() { crate::diagnostics::log_info(\"x\"); }" handlers/x.rs 'fn f() {}'
run_case core_uses_diagnostics fail "use crate::diagnostics;
$CORE_OK" handlers/x.rs 'fn f() {}'
run_case core_reads_ui_state fail "$CORE_OK
fn f(app: &Signex) { let _ = &app.ui_state; }" handlers/x.rs 'fn f() {}'
run_case core_reads_panel_ctx fail "$CORE_OK
fn f(s: &DocumentState) { let _ = &s.panel_ctx; }" handlers/x.rs 'fn f() {}'
run_case core_reads_window_panel_ctx fail "$CORE_OK
fn f(s: &Signex) { let _ = s.window_panel_ctx(); }" handlers/x.rs 'fn f() {}'
run_case core_reads_interaction fail "$CORE_OK
fn f(s: &Signex) { let _ = &s.interaction_state; }" handlers/x.rs 'fn f() {}'
run_case core_comment_only pass "// ui_state, panel_ctx and crate::diagnostics are named in prose only.
$CORE_OK" handlers/x.rs 'fn f() {}'

# A tree without the core module is an error, never a pass.
mkdir -p "$TMP/no_core/$APP"
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
