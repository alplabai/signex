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
#            wrapper, a delegating edit form, a load_gateway function that
#            writes core state, or an edit-tail wrapper listed in
#            scripts/sheet-change-wrappers.txt: a call, a path call, a method
#            or function named as a value, an import.
#   state    a `&mut self` method of DocumentState, called on
#            `<..>.document_state` or named as `DocumentState::<m>`. The names
#            are collected from every `impl DocumentState` block under
#            crates/signex-app/src, so a new one is caught without an edit
#            here; `&self` methods are reads and stay allowed.
#
# All read the code through ast-grep, so comments do not count. Test code is
# left out: files named `tests.rs` or `*_tests.rs`, anything under a `tests/`
# or `*_tests/` folder, and items under `#[cfg(test)]`. Blind spots: code
# inside a macro's token tree (`dbg!(..)`, `vec![..]`) is not parsed for the
# writes, though a listed name inside it is still caught; a `Signex` method
# or free function defined outside the projector that writes core state: only
# the listed and derived names are caught. The structural fix is #2713 (the
# projector takes `&DocumentState` at 4a).
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
# load_gateway functions that write core state: not load wrappers (#2705 names
# seven), but callers of clear_schematic_ui_state that stay out of the
# projector until 3.B splits that write.
CORE_WRITING_LOADS="apply_loaded_pcb_document apply_loaded_empty_document"

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
names="$(printf '%s\n' $MARKS $LOAD_WRAPPERS $DELEGATING $CORE_WRITING_LOADS $listed | sed '/^$/d' | sort -u | paste -sd '|' -)"

NOT_IN_TEST="
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
        pattern: '#[cfg(test)]'"

# Every `&mut self` method of an `impl DocumentState` block in the crate,
# whichever file declares it; the `&self` ones are reads and stay allowed.
if ! decls="$(ast-grep scan --json=compact --inline-rules "
id: document-state-mut-fn
language: rust
rule:
  kind: function_item
  has:
    field: parameters
    has:
      kind: self_parameter
      regex: '^&.*\\bmut\\s+self\$'
  inside:
    kind: impl_item
    stopBy: end
    has:
      field: type
      regex: '(^|::)DocumentState\$'" crates/signex-app/src)"; then
  echo "Projector write guard: FAIL — ast-grep could not list DocumentState's methods."
  exit 2
fi
state_names="$(jq -r '.[] | .text | capture("fn\\s+(?<n>[A-Za-z_][A-Za-z0-9_]*)").n' <<<"$decls" | sort -u | paste -sd '|' -)"
if [[ -z "$state_names" ]]; then
  echo "Projector write guard: FAIL — no \`&mut self\` method of DocumentState under crates/signex-app/src."
  exit 2
fi

if ! writes="$(ast-grep scan --rule "$RULE" --json=compact "$SRC")" \
  || ! uses="$(ast-grep scan --json=compact --inline-rules "
id: projector-name
language: rust
rule:
  any:
    - {kind: identifier, regex: '^($names)\$'}
    - {kind: field_identifier, regex: '^($names)\$'}
$NOT_IN_TEST" "$SRC")" \
  || ! state_uses="$(ast-grep scan --json=compact --inline-rules "
id: projector-document-state-mut
language: rust
rule:
  any:
    # \`<..>.document_state.<m>(..)\`
    - kind: field_expression
      all:
        - has: {field: field, regex: '^($state_names)\$'}
        - has:
            field: value
            kind: field_expression
            has: {field: field, regex: '^document_state\$'}
    # \`DocumentState::<m>\`, called or named as a value
    - kind: scoped_identifier
      all:
        - has: {field: path, regex: '(^|::)DocumentState\$'}
        - has: {field: name, regex: '^($state_names)\$'}
$NOT_IN_TEST" "$SRC")"; then
  echo "Projector write guard: FAIL — ast-grep could not scan $SRC."
  exit 2
fi
hits="$(jq -rs 'add | .[]
  | select(.file | test("(^|/|_)tests/|(^|/|_)tests\\.rs$") | not)
  | "\(.file):\(.range.start.line + 1): \(.text | split("\n")[0])"' <<<"$writes$uses$state_uses" | sort -u)"

if [[ -n "$hits" ]]; then
  echo "Projector write guard: FAIL — $SRC writes core state or calls a wrapper:"
  printf '%s\n' "$hits" | awk '{ print "  " $0 }'
  echo "  Do the write in the core half (app/state) and project its events here (#2326, #2705)."
  exit 1
fi
echo "Projector write guard: OK — no core-state write and no wrapper call in $SRC."
