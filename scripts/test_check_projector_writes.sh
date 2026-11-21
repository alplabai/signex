#!/usr/bin/env bash
#
# Controls for scripts/check_projector_writes.sh (#2326 slice 3, #2705): each
# write or wrapper call the guard must catch fails it, and each read it must
# leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_projector_writes.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0
SRC="crates/signex-app/src/app/projector"
LIST='# the edit-tail wrappers
handlers/editing_commands/sheet_on.rs after_sheet_change
read_only_refusal.rs refuse_if_read_only_at'

STATE='impl DocumentState {
    pub(crate) fn bump(&mut self) {}
    pub(crate) fn peek(&self) -> bool { true }
}'

# run_case <name> <expected: fail|pass> <file under projector> <body> [<file under crates/signex-app/src> <body>]
run_case() {
  local name="$1" expect="$2" tree="$TMP/$1" status=0
  mkdir -p "$tree/$SRC/$(dirname "$3")" "$tree/scripts" "$tree/crates/signex-app/src/app/state"
  printf '%s\n' "$LIST" > "$tree/scripts/sheet-change-wrappers.txt"
  printf '%s\n' "$STATE" > "$tree/crates/signex-app/src/app/state/document.rs"
  printf '%s\n' "$4" > "$tree/$SRC/$3"
  if [[ $# -ge 6 ]]; then
    mkdir -p "$tree/crates/signex-app/src/$(dirname "$5")"
    printf '%s\n' "$6" > "$tree/crates/signex-app/src/$5"
  fi
  PROJECTOR_WRITES_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

in_fn() { printf 'impl Signex {\n    fn f(&mut self, p: PathBuf, e: Engine) {\n        %s\n    }\n}' "$1"; }

# Reads pass.
run_case no_writes pass mod.rs "$(in_fn 'self.refresh_panel_ctx();')"
run_case active_path_reads pass slots.rs "$(in_fn 'let active_path = self.document_state.active_path.clone();
        let _ = self.document_state.active_path.as_deref() == Some(p.as_path());
        let _ = self.document_state.active_path.as_ref();
        let _ = self.document_state.active_path.is_some() || self.document_state.active_path.is_none();')"
run_case engine_map_reads pass slots.rs "$(in_fn 'let _ = self.document_state.engines.get(&p);
        let _ = self.document_state.engines.contains_key(&p);
        let _ = (self.document_state.engines.keys(), self.document_state.engines.values());
        let _ = (self.document_state.engines.iter(), self.document_state.engines.len());
        let _ = self.document_state.engines.is_empty();')"
run_case other_field_writes pass slots.rs "$(in_fn 'self.interaction_state.canvases.insert(id, slot);
        self.document_state.panel_ctx.paper_size = token;
        let s = &mut self.document_state.panel_ctx;')"
run_case comment_only pass slots.rs "// insert_engine, engines.remove, mark_saved and open_schematic_tab in prose only.
$(in_fn 'self.document_state.active_path.is_some();')"
run_case cfg_test_mod pass slots.rs "fn g() {}
#[cfg(test)]
mod tests { $(in_fn 'self.document_state.engines.clear(); self.apply_engine_undo();') }"
run_case tests_file pass slots_tests.rs "$(in_fn 'self.document_state.insert_engine(p, e);')"

# (a) A method on `.engines` or `.active_path` that is not a read.
for m in 'insert(p, e)' 'remove(&p)' 'entry(p).or_insert(e)' 'retain(|k, _| k != &p)' 'clear()' \
  'extend([(p, e)])' 'drain()' 'get_mut(&p).unwrap().execute(cmd)'; do
  run_case "engines_${m%%(*}" fail slots.rs "$(in_fn "self.document_state.engines.$m;")"
done
run_case engines_insert_split fail slots.rs "$(in_fn 'self.document_state
            .engines
            .insert(p, e);')"
for m in 'take()' 'replace(p)' 'insert(p)' 'get_or_insert(p)' 'clone_from(&None)'; do
  run_case "active_path_${m%%(*}" fail slots.rs "$(in_fn "self.document_state.active_path.$m;")"
done
# (b) `&mut` of the map, the active path or the document state.
run_case mem_take_engines fail slots.rs "$(in_fn 'let m = std::mem::take(&mut self.document_state.engines);')"
run_case mem_replace_active_path fail slots.rs "$(in_fn 'std::mem::replace(&mut self.document_state.active_path, Some(p));')"
run_case ufcs_insert fail slots.rs "$(in_fn 'HashMap::insert(&mut self.document_state.engines, p, e);')"
run_case alias_active_path fail slots.rs "$(in_fn 'let ap = &mut self.document_state.active_path; *ap = None;')"
run_case mut_document_state fail slots.rs "$(in_fn 'let d = &mut self.document_state;')"
# (c) Assignment to them.
run_case active_path_assign fail slots.rs "$(in_fn 'self.document_state.active_path = Some(p);')"
run_case engines_assign fail slots.rs "$(in_fn 'self.document_state.engines = HashMap::new();')"
run_case document_state_assign fail slots.rs "$(in_fn 'self.document_state = DocumentState::default();')"
run_case after_cfg_test_attr fail slots.rs "$(in_fn '#[cfg(test)] let _x = 1; self.document_state.active_path = None;')"
# Any other field reached through document_state, panel_ctx excepted.
run_case pcb_revisions_insert fail slots.rs "$(in_fn 'self.document_state.pcb_revisions.insert(p, 1);')"
run_case arbitrary_field_push fail slots.rs "$(in_fn 'self.document_state.foo.push(1);')"
run_case nested_field_method fail slots.rs "$(in_fn 'self.document_state.autosave.pending.clear();')"
run_case arbitrary_field_assign fail slots.rs "$(in_fn 'self.document_state.active_tab = 0;')"
run_case arbitrary_field_compound fail slots.rs "$(in_fn 'self.document_state.active_tab += 1;')"
run_case arbitrary_field_index_assign fail slots.rs "$(in_fn 'self.document_state.tabs[0].title = t;')"
run_case arbitrary_field_mut_ref fail slots.rs "$(in_fn 'let t = &mut self.document_state.tabs;')"
run_case arbitrary_field_reads pass slots.rs "$(in_fn 'let _ = self.document_state.tabs.get(i).map(|t| t.path.clone());
        let _ = (self.document_state.tabs.len(), self.document_state.tabs.iter(), self.document_state.tabs.first());
        let _ = (self.document_state.tabs.last(), self.document_state.pcb_revisions.get(&p).copied());
        let _ = (self.document_state.recovery_check_due.is_empty(), self.document_state.tabs.cloned());
        let tab = self.document_state.active_tab;')"
run_case panel_ctx_writes pass slots.rs "$(in_fn 'self.document_state.panel_ctx.paper_size = token;
        self.document_state.panel_ctx.sym_count += 1;
        self.document_state.panel_ctx.projects.push(row);
        self.document_state.panel_ctx.symbol_lists_key = None;
        let ctx = &mut self.document_state.panel_ctx;')"
# Marks and engine-map methods, as a call, a path call and a value.
for mark in insert_engine remove_engine clear_active_engine mark_document_dirty \
  mark_document_dirty_display_only mark_history_edit_dirty mark_document_clean \
  mark_landed_draft_clean clear_document_dirty mark_documents_clean_where \
  mark_installed_engine_saved mark_board_saved mark_saved forget_save_point mark_dirty set_dirty; do
  run_case "$mark" fail panel.rs "$(in_fn "self.document_state.$mark(&p);")"
done
run_case mark_by_path fail panel.rs "$(in_fn 'DocumentState::insert_engine(&mut ds, p, e);')"
run_case mark_as_value fail panel.rs "$(in_fn 'opt.map(Engine::mark_saved);')"
run_case mark_import fail panel.rs 'use crate::app::state::DocumentState::mark_document_dirty;'
run_case mark_in_macro fail panel.rs "$(in_fn 'dbg!(self.document_state.insert_engine(p, e));')"
# (d) Wrappers: load, delegating, and the edit-tail list file's names.
for w in open_schematic_tab install_reloaded_schematic restore_loaded apply_loaded_schematic \
  attach_parked_schematic_tab sync_visible_document_from_active_tab clear_schematic_ui_state \
  apply_engine_command apply_engine_command_checked apply_engine_command_reporting \
  apply_engine_command_unrecorded apply_engine_commands apply_engine_commands_checked \
  apply_engine_undo apply_engine_redo refuse_if_active_read_only \
  after_sheet_change refuse_if_read_only_at mark_active_document_dirty; do
  run_case "wrapper_$w" fail board_view.rs "$(in_fn "self.$w();")"
done
run_case wrapper_as_value fail board_view.rs "$(in_fn 'let f = Signex::apply_engine_undo;')"
run_case wrapper_suffix pass board_view.rs "$(in_fn 'self.reapply_engine_undo_marker();')"

# A tree without the projector or the wrapper list is an error, never a pass.
# `&mut self` methods of DocumentState, collected from the crate.
run_case state_mut_method fail slots.rs "$(in_fn 'self.document_state.bump();')"
run_case state_ref_method pass slots.rs "$(in_fn 'let _ = self.document_state.peek();')"
run_case state_mut_split fail slots.rs "$(in_fn 'self
            .document_state
            .bump();')"
run_case state_mut_by_path fail slots.rs "$(in_fn 'DocumentState::bump(&mut ds);')"
run_case state_mut_as_value fail slots.rs "$(in_fn 'let f = crate::app::state::DocumentState::bump;')"
run_case state_mut_other_file fail slots.rs "$(in_fn 'self.document_state.open_sheet(p);')" \
  app/state/document_load/open.rs 'impl DocumentState {
    pub(crate) fn open_sheet(&mut self, path: PathBuf) {}
}'
run_case state_mut_scoped_impl fail slots.rs "$(in_fn 'self.document_state.settle(p);')" \
  app/state/sheet_change/mod.rs "impl crate::app::state::DocumentState {
    pub(crate) fn settle<'a>(&'a mut self, path: PathBuf) {}
}"
run_case state_mut_name_elsewhere pass slots.rs "$(in_fn 'slot.bump(); self.bump();')"
run_case state_mut_in_test_mod pass slots.rs "fn g() {}
#[cfg(test)]
mod tests { $(in_fn 'self.document_state.bump();') }"

# No `&mut self` method of DocumentState found means a broken scan, not a pass.
tree="$TMP/no_state" status=0
mkdir -p "$tree/scripts" "$tree/$SRC"
printf '%s\n' "$LIST" > "$tree/scripts/sheet-change-wrappers.txt"
printf '%s\n' "$(in_fn 'self.refresh_panel_ctx();')" > "$tree/$SRC/mod.rs"
PROJECTOR_WRITES_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_state — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_state"
fi

for missing in projector list; do
  tree="$TMP/no_$missing" status=0
  mkdir -p "$tree/scripts" "$tree/crates"
  if [[ "$missing" == list ]]; then mkdir -p "$tree/$SRC"; else printf '%s\n' "$LIST" > "$tree/scripts/sheet-change-wrappers.txt"; fi
  PROJECTOR_WRITES_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ $status -ne 2 ]]; then
    echo "FAIL  no_$missing — expected exit 2, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    no_$missing"
  fi
done

if [[ $failures -ne 0 ]]; then
  echo "check_projector_writes controls: $failures case(s) failed"
  exit 1
fi
echo "check_projector_writes controls: OK"
