#!/usr/bin/env bash
#
# Controls for scripts/check_uifree_state.sh: each shape the gate claims to
# catch must fail it, and each shape it must leave alone must pass. Every case
# runs on a small synthetic signex-app tree with the real check_arch_guard.sh
# (DOMAIN is read from it), a rule file naming `Doc` and the listed UI-free
# module `app/clean.rs`, and the baseline the unmodified tree generates.
#
# Usage: scripts/test_check_uifree_state.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_uifree_state.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Rerun all controls once per awk at hand (BSD awk, plus mawk and gawk).
if [[ -z "${UIFREE_AWK:-}" ]]; then
  status=0 ran=0
  for awk_bin in /usr/bin/awk mawk gawk; do
    command -v "$awk_bin" >/dev/null 2>&1 || continue
    echo "== $awk_bin"
    ran=$((ran + 1))
    UIFREE_AWK="$awk_bin" bash "$0" || status=1
  done
  if (( ran == 0 )); then
    echo "FAIL  no awk found (looked for /usr/bin/awk, mawk, gawk)"
    exit 1
  fi
  exit "$status"
fi

failures=0

# Fields of `Doc` in the base tree: two of them count (iced, an unlisted
# signex-app module).
BASE_FIELDS='    pub plain: HashMap<String, Option<u64>>,
    pub domain: signex_types::schematic::Point,
    pub ui: HashMap<window::Id, u8>,
    pub cam: Camera,'

# make_tree <dir> <Doc fields> <extra production code of app/clean.rs>
#           [extra code after `Doc` in app/state.rs]
# `Doc` is scanned except its `skip_*` fields.
make_tree() {
  local t="$1" src="$1/crates/signex-app/src"
  mkdir -p "$t/scripts" "$src/app/core"
  cp "$ROOT/scripts/check_arch_guard.sh" "$t/scripts/"
  printf '%s\n' '[package]' 'name = "signex-app"' '' '[dependencies]' \
    'signex-types = { workspace = true }' 'iced = { workspace = true }' 'uuid = "1"' \
    > "$t/crates/signex-app/Cargo.toml"
  printf '%s\n' 'struct crates/signex-app/src/app/state.rs Doc *,!skip_*' \
    'module crates/signex-app/src/app/clean.rs  # listed UI-free' 'crate uuid' \
    > "$t/scripts/uifree-state.txt"
  printf '%s\n' 'pub mod app;' 'pub mod canvas;' > "$src/lib.rs"
  printf '%s\n' 'pub mod clean;' 'pub mod core;' 'pub mod state;' > "$src/app.rs"
  printf '%s\n' 'pub struct CoreId;' > "$src/app/core/id.rs"
  printf '%s\n' 'pub struct Camera;' 'pub fn fit() {}' > "$src/canvas.rs"
  printf '%s\n' 'use std::collections::HashMap;' 'use iced::window;' \
    'use crate::canvas::Camera;' '' 'pub struct Doc {' "$2" '}' "${4:-}" > "$src/app/state.rs"
  printf '%s\n' 'use std::path::PathBuf;' 'use signex_types::schematic::Point;' \
    'use crate::app::core::id::CoreId;' '' \
    'pub struct Clean { pub at: PathBuf, pub p: Point, pub id: CoreId }' '' \
    '#[cfg(test)]' 'mod tests {' '    use iced::Color;' '    use crate::canvas::Camera;' '}' \
    "$3" > "$src/app/clean.rs"
}

make_tree "$TMP/base" "$BASE_FIELDS" ''
UIFREE_ROOT="$TMP/base" bash "$GATE" --generate >/dev/null

# run_case <name> <pass|fail> <Doc fields> <clean.rs extra> [message] [state.rs extra]
# A fail case must print `message`; a pass case the OK line or `message`.
run_case() {
  local name="$1" expect="$2" want="${5:-}" tree="$TMP/$1" status=0 out
  make_tree "$tree" "$3" "$4" "${6:-}"
  cp "$TMP/base/scripts/uifree-state-baseline.txt" "$tree/scripts/"
  out="$(UIFREE_ROOT="$tree" bash "$GATE" 2>&1)" || status=$?
  [[ "$expect" == pass && -z "$want" ]] && want="UI-free state: OK"
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status: $out"
    failures=$((failures + 1))
  elif [[ "$out" != *"$want"* ]]; then
    echo "FAIL  $name — output lacks '$want': $out"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

# --- negative controls: the gate must pass these --------------------------

run_case base_tree pass "$BASE_FIELDS" '' 'OK — 2 counted fields (baseline 2), 1 UI-free modules'
run_case core_and_domain_fields pass "$BASE_FIELDS
    pub id: crate::app::core::id::CoreId,
    pub ids: Vec<uuid::Uuid>,
    pub at: Option<signex_types::schematic::Point>,
    pub n: [u8; 4]," ''
run_case listed_module_type pass "$BASE_FIELDS
    pub clean: crate::app::clean::Clean," ''
run_case below_baseline_passes pass '    pub plain: u64,
    pub ui: HashMap<window::Id, u8>,' '' 'below the baseline of 2. Lower it'
run_case listed_module_test_code_ignored pass "$BASE_FIELDS" '
#[cfg(test)]
fn helper() { let _ = crate::canvas::fit(); }'
run_case excluded_field_glob pass "$BASE_FIELDS
    pub skip_colour: iced::Color," ''
run_case fn_scope_use_does_not_count pass "$BASE_FIELDS
    pub at: Point," '' '' 'use signex_types::schematic::Point;
fn to_screen() { use iced::Point; let _ = Point::ORIGIN; }'
run_case listed_module_uses_core_and_std pass "$BASE_FIELDS" '
pub fn f() -> std::path::PathBuf { let _ = crate::app::core::id::CoreId; PathBuf::new() }'

# --- positive controls: the gate must flag these ---------------------------

run_case iced_field fail "$BASE_FIELDS
    pub colour: iced::Color," '' 'state.rs:10 Doc.colour: iced::Color (iced)'
run_case iced_generic_argument fail "$BASE_FIELDS
    pub colours: Vec<iced::Color>," '' 'Doc.colours: Vec<iced::Color> (iced)'
run_case iced_through_use_alias fail "$BASE_FIELDS
    pub w: window::Settings," '' 'Doc.w: window::Settings (iced)'
run_case unlisted_app_type fail "$BASE_FIELDS
    pub cam2: Option<Camera>," '' 'Doc.cam2: Option<Camera> (crate::canvas)'
run_case same_file_type_counts fail "$BASE_FIELDS
    pub doc: Box<Doc>," '' 'Doc.doc: Box<Doc> (crate::app::state)'
run_case super_path_resolves fail "$BASE_FIELDS
    pub cam3: super::super::canvas::Camera," '' '(crate::canvas)'
run_case unresolved_ident_counts fail "$BASE_FIELDS
    pub thing: Mystery," '' 'Doc.thing: Mystery (unresolved)'
run_case glob_scans_other_fields fail "$BASE_FIELDS
    pub scan_colour: iced::Color," '' 'Doc.scan_colour: iced::Color (iced)'
run_case fn_scope_use_does_not_rebind fail "$BASE_FIELDS
    pub at: Point," '' 'Doc.at: Point (iced)' 'use iced::Point;
fn to_doc() {
    use signex_types::schematic::Point;
    let _ = Point::default();
}'
run_case inline_mod_use_does_not_rebind fail "$BASE_FIELDS
    pub c: Color," '' 'Doc.c: Color (iced)' 'use iced::Color;
mod convert {
    use std::string::String as Color;
    pub fn f(_: Color) {}
}'
run_case name_bound_twice_is_unresolved fail "$BASE_FIELDS
    pub c: Color," '' 'Doc.c: Color (unresolved)' '#[cfg(feature = "a")]
use signex_types::Color;
#[cfg(not(feature = "a"))]
use iced::Color;'
run_case swapped_field_fails fail '    pub plain: u64,
    pub ui: HashMap<window::Id, u8>,
    pub colour: iced::Color,' '' 'Doc.colour: iced::Color (iced)'
run_case listed_module_imports_iced fail "$BASE_FIELDS" 'use iced::Color;' \
  'clean.rs:12 imports iced::Color (iced)'
run_case listed_module_imports_gui_module fail "$BASE_FIELDS" 'use crate::canvas::Camera;' \
  'clean.rs:12 imports crate::canvas::Camera (crate::canvas)'
run_case listed_module_super_to_gui fail "$BASE_FIELDS" 'use super::state::Doc;' \
  'imports crate::app::state::Doc (crate::app::state)'
run_case listed_module_names_gui_inline fail "$BASE_FIELDS" 'pub fn f() { crate::canvas::fit(); }' \
  'names crate::canvas::fit (crate::canvas)'
run_case listed_module_names_iced_inline fail "$BASE_FIELDS" 'pub fn f() -> iced::Color { todo!() }' \
  'names iced::Color (iced)'
run_case listed_module_const_does_not_shadow_crate fail "$BASE_FIELDS" '#[allow(non_upper_case_globals)]
const iced: () = ();
pub fn f() -> iced::Color { todo!() }' 'names iced::Color (iced)'
run_case listed_module_imports_crate_root fail "$BASE_FIELDS" 'use crate as root;' \
  'imports crate (crate)'

# --- and the real tree passes ----------------------------------------------

if UIFREE_AWK="$UIFREE_AWK" bash "$GATE" >/dev/null 2>&1; then
  echo "ok    real_tree"
else
  echo "FAIL  real_tree — the gate rejects the current workspace"
  failures=$((failures + 1))
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All UI-free state controls passed."
