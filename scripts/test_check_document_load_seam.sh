#!/usr/bin/env bash
#
# Controls for scripts/check_document_load_seam.sh (#2326 slice 3, issue
# #2704): each shape the guard must catch fails it, and each shape it must
# leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_document_load_seam.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0
APP="crates/signex-app/src/app"

DROP_ARM='fn drop_stale(s: &mut DocumentState, r: Result<u8, u8>) {
    match r {
        Ok(_) => {}
        Err(_) => {
            s.engines.remove(&1);
        }
    }
}'
CORE_OK="pub(crate) fn install_active(&mut self) -> bool { true }
pub(super) fn helper_notice() {}
pub(self) fn self_only() {}
fn private_step() {}
$DROP_ARM"
LIST_OK='# the exception
load_gateway.rs apply_loaded_schematic'
WRAPPER='impl Signex {
    fn apply_loaded_schematic(&mut self) { let _ = self.document_state.install_active(); }
}'
CALL='fn f(app: &mut Signex) { let _ = app.document_state.install_active(); }'

# run_case <name> <expected: fail|pass> <core mod.rs> <allowlist> <load_gateway.rs> [<file under app> <body>]
# The trees allow one wrapper, the one LIST_OK names (its `cksum` below).
run_case() {
  local name="$1" expect="$2" tree="$TMP/$1" status=0
  mkdir -p "$tree/$APP/state/document_load" "$tree/scripts"
  printf '%s\n' "$3" > "$tree/$APP/state/document_load/mod.rs"
  printf '%s\n' "$4" > "$tree/scripts/document-load-wrappers.txt"
  printf '%s\n' "$5" > "$tree/$APP/load_gateway.rs"
  if [[ $# -ge 7 ]]; then
    mkdir -p "$tree/$APP/$(dirname "$6")"
    printf '%s\n' "$7" > "$tree/$APP/$6"
  fi
  DOCUMENT_LOAD_SEAM_MAX=1 DOCUMENT_LOAD_SEAM_CKSUM=3247644973 DOCUMENT_LOAD_SEAM_ROOT="$tree" \
    bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

run_case listed_wrapper pass "$CORE_OK" "$LIST_OK" "$WRAPPER"
# Callers outside the wrapper list.
run_case extra_caller fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/erc/erc_run.rs "$CALL"
run_case other_fn_in_wrapper_file fail "$CORE_OK" "$LIST_OK" "$WRAPPER
$CALL"
run_case by_path fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'fn f() { document_load::install_active(); }'
run_case path_as_value fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'fn f(v: Vec<DocumentState>) { let _ = v.into_iter().map(DocumentState::install_active); }'
run_case import fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'use crate::app::state::document_load::install_active;'
run_case in_assert_macro fail "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'fn f(s: &mut DocumentState) { assert!(s.install_active()); }'
run_case pub_super fail "$CORE_OK" "$LIST_OK" "$WRAPPER" state/other.rs 'fn f() { helper_notice(); }'
run_case pub_self pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'fn f() { self_only(); private_step(); }'
run_case field_of_that_name pass "$CORE_OK" "$LIST_OK" "$WRAPPER" handlers/open.rs 'fn f(s: &S) -> bool { s.install_active }'
# Test code may call the core half.
run_case cfg_test_fn pass "$CORE_OK" "$LIST_OK" "$WRAPPER
impl Signex {
    /// A test seam.
    #[cfg(test)]
    fn install_active_engine(&mut self) -> bool { self.document_state.install_active() }
}"
run_case tests_file pass "$CORE_OK" "$LIST_OK" "$WRAPPER" load_seam_tests.rs "$CALL"
run_case tests_dir pass "$CORE_OK" "$LIST_OK" "$WRAPPER" state/document_load/tests/open.rs "$CALL"
# The list: shrink-only, names pinned, no stale line.
run_case list_grows fail "$CORE_OK" "$LIST_OK
handlers/open.rs f" "$WRAPPER" handlers/open.rs "$CALL"
run_case list_swaps_a_name fail "$CORE_OK" 'load_gateway.rs other_fn' 'impl Signex {
    fn other_fn(&mut self) { let _ = self.document_state.install_active(); }
}'
run_case list_below_max fail "$CORE_OK" '# empty' 'fn g() {}'
run_case stale_line fail "$CORE_OK" "$LIST_OK" 'impl Signex {
    fn apply_loaded_schematic(&mut self) { self.refresh(); }
}'
# No trait and no trait impl in the core module.
run_case trait_in_core fail "$CORE_OK
pub(crate) trait Reach { fn reach(&mut self) {} }" "$LIST_OK" "$WRAPPER"
run_case trait_impl_in_core fail "$CORE_OK
impl From<u8> for SheetInstall { fn from(_: u8) -> Self { todo!() } }" "$LIST_OK" "$WRAPPER"
run_case inherent_impl_in_core pass "$CORE_OK
impl DocumentState { fn helper(&self) {} }" "$LIST_OK" "$WRAPPER"
# No GUI state, no diagnostics, no catch_unwind, no direct-logging helper.
run_case core_logs fail "$CORE_OK
fn log() { crate::diagnostics::log_warning(\"x\"); }" "$LIST_OK" "$WRAPPER"
run_case core_reads_interaction fail "$CORE_OK
fn f(s: &Signex) { let _ = &s.interaction_state; }" "$LIST_OK" "$WRAPPER"
run_case core_reads_ui_state fail "$CORE_OK
fn f(s: &Signex) { let _ = &s.ui_state; }" "$LIST_OK" "$WRAPPER"
run_case core_reads_panel_ctx fail "$CORE_OK
fn f(s: &DocumentState) { let _ = &s.panel_ctx; }" "$LIST_OK" "$WRAPPER"
run_case core_reads_window_panel_ctx fail "$CORE_OK
fn f(s: &Signex) { let _ = s.window_panel_ctx(); }" "$LIST_OK" "$WRAPPER"
run_case core_catch_unwind fail "$CORE_OK
fn f() { let _ = std::panic::catch_unwind(|| ()); }" "$LIST_OK" "$WRAPPER"
for helper in log_sheet_repairs report_bare_pin_taps report_unrecognized_sheet_size warn_if_read_only; do
  run_case "core_calls_$helper" fail "$CORE_OK
fn f(p: &Path) { crate::app::$helper(p); }" "$LIST_OK" "$WRAPPER"
done
run_case core_comment_only pass "// ui_state, catch_unwind and log_sheet_repairs are named in prose only.
$CORE_OK" "$LIST_OK" "$WRAPPER"
# No closure parameter.
run_case core_impl_fnonce fail "$CORE_OK
pub(crate) fn finish(&mut self, f: impl FnOnce(&mut Engine) -> bool) {}" "$LIST_OK" "$WRAPPER"
run_case core_generic_fnmut fail "$CORE_OK
fn each<F>(f: F) where F: FnMut(u8) {}" "$LIST_OK" "$WRAPPER"
run_case core_fn_pointer fail "$CORE_OK
fn call(f: fn(u8) -> u8) {}" "$LIST_OK" "$WRAPPER"
# Exactly one engine removal, in an Err arm.
run_case second_removal fail "$CORE_OK
fn again(s: &mut DocumentState) { match 1 { _ => { s.engines.remove(&2); } } }" "$LIST_OK" "$WRAPPER"
run_case removal_outside_err fail 'pub(crate) fn install_active(&mut self) -> bool { true }
fn drop_stale(s: &mut DocumentState, r: Result<u8, u8>) {
    match r {
        Ok(_) => {
            s.engines.remove(&1);
        }
        Err(_) => {}
    }
}' "$LIST_OK" "$WRAPPER"
run_case no_removal fail 'pub(crate) fn install_active(&mut self) -> bool { true }' "$LIST_OK" "$WRAPPER"
run_case removal_in_test pass "$CORE_OK
#[cfg(test)]
mod tests { fn t(s: &mut DocumentState) { s.engines.remove(&3); } }" "$LIST_OK" "$WRAPPER"

# A tree without the core module is an error, never a pass.
mkdir -p "$TMP/no_core/$APP" "$TMP/no_core/scripts"
printf '%s\n' "$LIST_OK" > "$TMP/no_core/scripts/document-load-wrappers.txt"
status=0
DOCUMENT_LOAD_SEAM_ROOT="$TMP/no_core" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_core — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_core"
fi

if [[ $failures -ne 0 ]]; then
  echo "check_document_load_seam controls: $failures case(s) failed"
  exit 1
fi
echo "check_document_load_seam controls: OK"
