#!/usr/bin/env bash
#
# Projector write guard (#2326 slice 3, ADR-0006 and #2705): crates/signex-app/
# src/app/projector/ only draws documents into the GUI. It writes no core
# state and calls no wrapper:
#
#   writes   (scripts/projector-writes.yml) a method on `.engines` other than
#            get, contains_key, keys, values, iter, len, is_empty; a method on
#            `.active_path` other than clone, as_deref, as_ref, is_some,
#            is_none; a `&mut` of, or an assignment to, `.engines`,
#            `.active_path` or `.document_state`; and for any other field
#            reached through `document_state` but `panel_ctx` (the GUI's own
#            projection), a `&mut`, an assignment or a method other than the
#            reads above and copied, cloned, first, last.
#   names    any use of a dirty or saved mark, an engine-map method, a load
#            wrapper, a delegating edit form, or an edit-tail wrapper listed in
#            scripts/sheet-change-wrappers.txt: a call, a path call, a method
#            or function named as a value, an import.
#
# Both read the code through ast-grep, so comments do not count. Test code is
# left out: files named `tests.rs` or `*_tests.rs`, anything under a `tests/`
# or `*_tests/` folder, and items under `#[cfg(test)]`. Blind spots: code
# inside a macro's token tree (`dbg!(..)`, `vec![..]`) is not parsed for the
# writes, though a listed name inside it is still caught.
#
# Usage: scripts/check_projector_writes.sh   (PROJECTOR_WRITES_ROOT overrides
# the tree, for scripts/test_check_projector_writes.sh)
set -euo pipefail

ROOT="${PROJECTOR_WRITES_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
RULE="$(cd "$(dirname "$0")" && pwd)/projector-writes.yml"
SRC="crates/signex-app/src/app/projector"
LIST="scripts/sheet-change-wrappers.txt"

# Dirty and saved marks, and the engine-map methods of DocumentState.
MARKS="insert_engine remove_engine clear_active_engine mark_saved forget_save_point
  mark_dirty set_dirty mark_document_dirty mark_document_dirty_display_only
  mark_history_edit_dirty mark_document_clean mark_landed_draft_clean
  clear_document_dirty mark_documents_clean_where mark_active_document_dirty
  mark_installed_engine_saved mark_board_saved"
# The load wrappers (#2705; they end in slices 5b and 6).
LOAD_WRAPPERS="open_schematic_tab install_reloaded_schematic restore_loaded
  apply_loaded_schematic attach_parked_schematic_tab
  sync_visible_document_from_active_tab clear_schematic_ui_state"
# The forms that delegate to the edit-tail wrappers.
DELEGATING="apply_engine_command apply_engine_command_checked
  apply_engine_command_reporting apply_engine_command_unrecorded
  apply_engine_commands apply_engine_commands_checked apply_engine_undo
  apply_engine_redo refuse_if_active_read_only"

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Projector write guard: FAIL — $tool is not installed."
    exit 2
  fi
done
if [[ ! -d "$SRC" || ! -f "$LIST" ]]; then
  echo "Projector write guard: FAIL — no $SRC or $LIST under $ROOT."
  exit 2
fi

listed="$(grep -vE '^[[:space:]]*(#|$)' "$LIST" | awk '{ print $2 }')"
names="$(printf '%s\n' $MARKS $LOAD_WRAPPERS $DELEGATING $listed | sed '/^$/d' | sort -u | paste -sd '|' -)"

if ! writes="$(ast-grep scan --rule "$RULE" --json=compact "$SRC")" \
  || ! uses="$(ast-grep scan --json=compact --inline-rules "
id: projector-name
language: rust
rule:
  any:
    - {kind: identifier, regex: '^($names)\$'}
    - {kind: field_identifier, regex: '^($names)\$'}
  not:
    inside:
      stopBy: end
      any:
        - kind: mod_item
        - kind: function_item
        - kind: impl_item
      follows:
        stopBy:
          not:
            any:
              - kind: attribute_item
              - kind: line_comment
              - kind: block_comment
        pattern: '#[cfg(test)]'" "$SRC")"; then
  echo "Projector write guard: FAIL — ast-grep could not scan $SRC."
  exit 2
fi
hits="$(jq -rs 'add | .[]
  | select(.file | test("(^|/|_)tests/|(^|/|_)tests\\.rs$") | not)
  | "\(.file):\(.range.start.line + 1): \(.text | split("\n")[0])"' <<<"$writes$uses" | sort -u)"

if [[ -n "$hits" ]]; then
  echo "Projector write guard: FAIL — $SRC writes core state or calls a wrapper:"
  printf '%s\n' "$hits" | awk '{ print "  " $0 }'
  echo "  Do the write in the core half (app/state) and project its events here (#2326, #2705)."
  exit 1
fi
echo "Projector write guard: OK — no core-state write and no wrapper call in $SRC."
