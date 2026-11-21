#!/usr/bin/env bash
#
# Controls for scripts/check_projector_writes.sh (#2326 slice 3): each write
# the guard must catch fails it, and each read it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_projector_writes.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0
SRC="crates/signex-app/src/app/projector"

# run_case <name> <expected: fail|pass> <file under projector> <body>
run_case() {
  local name="$1" expect="$2" tree="$TMP/$1" status=0
  mkdir -p "$tree/$SRC/$(dirname "$3")"
  printf '%s\n' "$4" > "$tree/$SRC/$3"
  PROJECTOR_WRITES_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

in_fn() { printf 'impl Signex {\n    fn f(&mut self, p: PathBuf, e: Engine) {\n        %s\n    }\n}' "$1"; }

run_case no_writes pass mod.rs "$(in_fn 'self.refresh_panel_ctx();')"
# Reads of the active path, as the projector does them today.
run_case active_path_reads pass slots.rs "$(in_fn 'let active_path = self.document_state.active_path.clone();
        let is_active = self.document_state.active_path.as_deref() == Some(p.as_path());')"
run_case engine_map_reads pass slots.rs "$(in_fn 'let _ = self.document_state.engines.get(&p);
        let _ = self.document_state.engines.contains_key(&p);')"
run_case other_map_insert pass slots.rs "$(in_fn 'self.interaction_state.canvases.insert(id, slot);')"
run_case comment_only pass slots.rs "// insert_engine, engines.remove and mark_saved are named in prose only.
$(in_fn 'self.document_state.active_path.is_some();')"
run_case cfg_test_mod pass slots.rs "fn g() {}
#[cfg(test)]
mod tests { $(in_fn 'self.document_state.mark_document_dirty(&p);') }"
run_case tests_file pass slots_tests.rs "$(in_fn 'self.document_state.insert_engine(p, e);')"

# Engine-map writes.
run_case insert_engine fail slots.rs "$(in_fn 'self.document_state.insert_engine(p, e);')"
run_case insert_engine_by_path fail slots.rs "$(in_fn 'DocumentState::insert_engine(&mut self.document_state, p, e);')"
run_case remove_engine fail slots.rs "$(in_fn 'let _ = self.document_state.remove_engine(&p);')"
run_case clear_active_engine fail slots.rs "$(in_fn 'self.document_state.clear_active_engine();')"
run_case engines_insert fail slots.rs "$(in_fn 'self.document_state.engines.insert(p, e);')"
run_case engines_insert_split fail slots.rs "$(in_fn 'self.document_state
            .engines
            .insert(p, e);')"
run_case engines_remove fail slots.rs "$(in_fn 'self.document_state.engines.remove(&p);')"
# Active path.
run_case active_path_assign fail slots.rs "$(in_fn 'self.document_state.active_path = Some(p);')"
run_case active_path_clear fail board_view.rs "$(in_fn 'self.document_state.active_path = None;')"
# Dirty and saved marks.
for mark in mark_document_dirty mark_document_dirty_display_only mark_history_edit_dirty \
  mark_document_clean mark_landed_draft_clean clear_document_dirty mark_installed_engine_saved \
  mark_board_saved; do
  run_case "$mark" fail panel.rs "$(in_fn "self.document_state.$mark(&p);")"
done
run_case mark_active_document_dirty fail panel.rs "$(in_fn 'self.mark_active_document_dirty();')"
run_case mark_documents_clean_where fail panel.rs "$(in_fn 'self.document_state.mark_documents_clean_where(|_| true);')"
run_case mark_saved fail panel.rs "$(in_fn 'self.document_state.engines.get_mut(&p).unwrap().mark_saved();')"
run_case forget_save_point fail panel.rs "$(in_fn 'e.forget_save_point();')"
run_case mark_dirty fail panel.rs "$(in_fn 'self.tab_mut().mark_dirty();')"
run_case set_dirty fail panel.rs "$(in_fn 'self.tab_mut().set_dirty(true);')"

# A tree without the projector is an error, never a pass.
mkdir -p "$TMP/no_projector/crates"
status=0
PROJECTOR_WRITES_ROOT="$TMP/no_projector" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_projector — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_projector"
fi

if [[ $failures -ne 0 ]]; then
  echo "check_projector_writes controls: $failures case(s) failed"
  exit 1
fi
echo "check_projector_writes controls: OK"
