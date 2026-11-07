#!/usr/bin/env bash
#
# Controls for scripts/check_layer_ratchet.sh: each shape the gate claims to
# catch must fail it, and each shape it must leave alone must pass. A grep
# gate that is never shown a violation can go dead and still say "OK".
#
# Usage: scripts/test_check_layer_ratchet.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_layer_ratchet.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

# put <tree> <relative path> <body>
put() {
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
}

# new_tree <name> — a clean tree with a zero baseline; echoes its path.
new_tree() {
  local tree="$TMP/$1"
  mkdir -p "$tree/scripts" "$tree/crates"
  put "$tree" scripts/layer-ratchet-baseline.txt 'symbol_transform_outside_types=0
union_find_outside_net=0
app_library_editor_internals=0'
  echo "$tree"
}

# expect <name> <fail|pass> <tree> [gate args]
expect() {
  local name="$1" want="$2" tree="$3" status=0
  shift 3
  LAYER_RATCHET_ROOT="$tree" bash "$GATE" "$@" >/dev/null 2>&1 || status=$?
  if [[ "$want" == fail && $status -ne 1 ]] || [[ "$want" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $want, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

# --- positive controls: the gate must flag these -------------------------

t="$(new_tree symbol_transform_copy)"
put "$t" crates/signex-app/src/x.rs 'pub struct SymbolTransform { a: u8 }'
expect symbol_transform_copy fail "$t"

t="$(new_tree union_find_named_helper)"
put "$t" crates/signex-erc/src/x.rs 'fn uf_find(parent: &mut [usize], x: usize) -> usize { x }'
expect union_find_named_helper fail "$t"

t="$(new_tree union_find_path_halving)"
put "$t" crates/signex-bom/src/x.rs 'fn go() { parent[x] = parent[parent[x]]; }'
expect union_find_path_halving fail "$t"

t="$(new_tree editor_internals_grew)"
put "$t" crates/signex-app/src/app/x.rs 'use crate::library::editor::footprint::state::EditorMode;'
expect editor_internals_grew fail "$t"

t="$(new_tree editor_internals_shrank_without_regenerate)"
put "$t" scripts/layer-ratchet-baseline.txt 'symbol_transform_outside_types=0
union_find_outside_net=0
app_library_editor_internals=3'
put "$t" crates/signex-app/src/app/x.rs 'use crate::library::editor::symbol::canvas::SymbolTool;'
expect editor_internals_shrank_without_regenerate fail "$t"

t="$(new_tree missing_baseline)"
rm "$t/scripts/layer-ratchet-baseline.txt"
expect missing_baseline fail "$t"

# --- negative controls: the gate must leave these alone ------------------

t="$(new_tree owners_may_hold_their_code)"
put "$t" crates/signex-types/src/x.rs 'pub struct SymbolTransform { a: u8 }'
put "$t" crates/signex-net/src/uf.rs 'pub fn find(parent: &mut HashMap<K, K>, x: K) -> K { x }'
put "$t" crates/signex-app/src/library/editor/a.rs 'use crate::library::editor::footprint::state::EditorMode;'
expect owners_may_hold_their_code pass "$t"

t="$(new_tree editor_public_surface_is_fine)"
put "$t" crates/signex-app/src/app/x.rs 'use crate::library::editor::footprint::FootprintEditorState;'
expect editor_public_surface_is_fine pass "$t"

t="$(new_tree baseline_matches)"
put "$t" scripts/layer-ratchet-baseline.txt 'symbol_transform_outside_types=0
union_find_outside_net=0
app_library_editor_internals=1'
put "$t" crates/signex-app/src/app/x.rs 'use crate::library::editor::symbol::state::SymbolSelection;'
expect baseline_matches pass "$t"

# --generate rewrites the baseline so a shrunk tree passes again.
t="$(new_tree generate_relocks)"
put "$t" scripts/layer-ratchet-baseline.txt 'symbol_transform_outside_types=0
union_find_outside_net=0
app_library_editor_internals=5'
expect generate_before fail "$t"
expect generate_run pass "$t" --generate
expect generate_after pass "$t"

if (( failures )); then
  echo "check_layer_ratchet controls: $failures case(s) failed"
  exit 1
fi
echo "check_layer_ratchet controls: OK"
