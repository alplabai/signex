#!/usr/bin/env bash
#
# Controls for scripts/check_core_guard.sh: each shape the gate claims to catch
# must fail it, and each shape it must leave alone must pass. A grep gate that
# is never shown a violation can go dead and still say "OK".
#
# Usage: scripts/test_check_core_guard.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_core_guard.sh"
CORE_REL="crates/signex-app/src/app/core"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# The scanner must hold under every POSIX awk at hand: rerun all controls once
# per implementation (BSD awk, plus mawk and gawk when installed).
if [[ -z "${CORE_GUARD_AWK:-}" ]]; then
  status=0 ran=0
  for awk_bin in /usr/bin/awk mawk gawk; do
    command -v "$awk_bin" >/dev/null 2>&1 || continue
    echo "== $awk_bin"
    ran=$((ran + 1))
    CORE_GUARD_AWK="$awk_bin" bash "$0" || status=1
  done
  if (( ran == 0 )); then
    echo "FAIL  no awk found (looked for /usr/bin/awk, mawk, gawk)"
    exit 1
  fi
  exit "$status"
fi

failures=0

# run_case <name> <fail|pass> <core file body | -none- | -empty-> [message] [file]
#
# `file` is the path below app/core/ (default sub/x.rs, module depth 2);
# `../core.rs` is the sibling module root instead of the directory.
# The exit code alone is not enough: a gate that crashes under `set -e` also
# exits 1. A fail case must print `message`; a pass case the OK line.
run_case() {
  local name="$1" expect="$2" body="$3" want="${4:-}" file="${5:-sub/x.rs}" tree="$TMP/$1" status=0 out target
  mkdir -p "$tree/crates/signex-app/src/app"
  target="$tree/$CORE_REL/$file"
  [[ "$file" == ../* ]] && target="$tree/crates/signex-app/src/app/${file#../}"
  case "$body" in
    -none-) ;;
    -empty-) mkdir -p "$tree/$CORE_REL" ;;
    *) mkdir -p "$(dirname "$target")"; printf '%s\r\n' "$body" > "$target" ;;
  esac
  out="$(CORE_GUARD_ROOT="$tree" bash "$GATE" 2>&1)" || status=$?
  [[ "$expect" == pass ]] && want="Core guard: OK"
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

run_case core_dir_absent pass -none-
run_case core_dir_empty pass -empty-
run_case clean_core pass 'pub struct AppCore { pub open: Vec<String> }'
run_case crate_path_inside_core pass 'use crate::app::core::state::CoreState;'
run_case crate_core_module_itself pass 'use crate::app::core;'
run_case crate_core_group pass 'use crate::app::core::{effects, state};'
run_case comment_names_banned_things pass '// no iced, tokio, std::fs or crate::app::view here
/// spawn_blocking stays in the shell
    //! std::thread is not used'
run_case similar_words_are_not_bans pass 'let sliced = 1; let tokio_free = 2; let fsx = 3;'

# --- positive controls: the gate must flag these ---------------------------

run_case crate_path_outside_core fail 'use crate::app::view::Theme;' \
  "x.rs:1 breaks 'crate_outside_core'"
run_case crate_core_prefix_is_not_core fail 'use crate::app::core_extra::X;' \
  "breaks 'crate_outside_core'"
run_case crate_path_after_core_path fail 'use crate::app::core::a; use crate::keymap::K;' \
  "breaks 'crate_outside_core'"
run_case iced_import fail 'use iced::Task;' "breaks 'iced'"
run_case iced_subcrate fail 'use iced_aw::Menu;' "breaks 'iced'"
run_case tokio_use fail 'tokio::spawn(async {});' "breaks 'tokio'"
run_case spawn_blocking fail 'let h = spawn_blocking(move || 1);' "breaks 'blocking'"
run_case run_blocking fail 'run_blocking(job);' "breaks 'blocking'"
run_case std_fs fail 'let s = std::fs::read_to_string(p)?;' "breaks 'std_fs'"
run_case std_fs_in_group fail 'use std::{fs, io};' "breaks 'std_fs'"
run_case std_thread fail 'std::thread::sleep(d);' "breaks 'std_thread'"
run_case std_process fail 'use std::process::Command;' "breaks 'std_process'"
run_case reports_line_number fail 'pub fn ok() {}
use tokio::sync::mpsc;' "x.rs:2 breaks 'tokio'"

# --- multi-line use groups are judged like the one-line form --------------

run_case multiline_std_group fail 'use std::{
    fs, // read the file
};' "x.rs:1 breaks 'std_fs': use std::{
    matched: use std::fs;"
run_case use_after_attribute_is_joined fail '#[cfg(unix)] use std::{
    fs };' "x.rs:1 breaks 'std_fs'"
run_case use_after_brace_is_joined fail 'fn f() { use std::{
    fs}; }' "x.rs:1 breaks 'std_fs'"
run_case use_after_statement_is_joined fail 'use a::b; use std::{
    fs };' "x.rs:1 breaks 'std_fs'"
run_case multiline_crate_group_inside_core pass 'pub(crate) use crate::{
    app::core::x,
    app::core::{y, z},
};'
run_case multiline_crate_group_escapes fail 'use crate::{
    app::core::x,
    app::other,
};' "matched: use crate::app::other;"
run_case nested_crate_group_escapes fail 'use crate::app::{core::x, view::Y};' \
  "matched: use crate::app::view::Y;"
run_case second_statement_on_a_line fail 'use crate::app::core::a; use crate::keymap::K;' \
  "breaks 'crate_outside_core'"

# --- aliasing the crate root ------------------------------------------------

run_case use_crate_as fail 'use crate as root;' "breaks 'crate_alias'"
run_case use_crate_self_as fail 'use crate::{self as root};' "breaks 'crate_alias'"
run_case extern_crate_self_as fail 'extern crate self as root;' "breaks 'crate_alias'"

# --- aliasing std / core / alloc --------------------------------------------

run_case use_std_as fail 'use std as s; fn f(p: &str) { s::fs::read(p); }' "breaks 'std_alias'"
run_case extern_crate_std_as fail 'extern crate std as s;' "breaks 'std_alias'"
run_case use_std_self_as fail 'use ::std::{self as s};' "breaks 'std_alias'"
run_case use_core_as fail 'use core as c;' "breaks 'std_alias'"
run_case core_module_self_as_is_fine pass 'use crate::app::core::{self as app_core};'

# --- outside source pulled in without crate:: -------------------------------

run_case path_attribute fail '#[path = "../../view/mod.rs"]
mod v;' "x.rs:1 breaks 'path_attr'"
run_case path_in_cfg_attr fail '#[cfg_attr(test, path = "../../view/mod.rs")]
mod v;' "breaks 'path_attr'"
run_case path_words_in_other_attributes_are_fine pass '#[doc = "see the path = config"]
#[error("bad path = {0}")]
struct E;'
run_case include_macro fail 'include!("../../view/theme.rs");' "breaks 'include_source'"
run_case include_str_data_is_fine pass 'const T: &str = include_str!("table.tsv");
const B: &[u8] = include_bytes!("blob.bin");'

# --- trailing comments are not code -----------------------------------------

run_case trailing_comment_is_skipped pass 'let a = 1; // like crate::app::view or std::fs'
run_case slashes_in_a_string_are_not_a_comment fail 'let u = "a//b"; let s = std::fs::read(u);' \
  "breaks 'std_fs'"
run_case trailing_comment_reports_original_line fail 'let s = std::fs::read(p); // why' \
  "breaks 'std_fs': let s = std::fs::read(p); // why"

# --- super:: chains against the module depth --------------------------------

run_case super_inside_core pass 'use super::super::state::CoreState;' '' sub/x.rs
run_case super_escapes_core fail 'use super::super::super::view::Theme;' \
  "x.rs:1 breaks 'super_escape'" sub/x.rs
run_case super_escapes_from_mod_rs fail 'use super::super::x;' \
  "mod.rs:1 breaks 'super_escape'" sub/mod.rs
run_case super_inside_core_from_mod_rs pass 'use super::x;' '' sub/mod.rs
run_case super_from_core_root_mod_rs fail 'pub use super::view;' \
  "breaks 'super_escape'" mod.rs
run_case super_in_group_escapes fail 'use super::{
    super::super::outside,
};' "breaks 'super_escape'" sub/x.rs
run_case super_in_inline_test_mod pass '#[cfg(test)]
mod tests {
    use super::*;
    fn f() { let _ = 1; }
}' '' mod.rs
run_case super_after_inline_mod_closes fail 'mod tests {
    use super::*;
}
use super::view;' "mod.rs:4 breaks 'super_escape'" mod.rs

# --- the sibling module root app/core.rs ------------------------------------

run_case core_rs_std_fs fail 'use std::fs;' "core.rs:1 breaks 'std_fs'" ../core.rs
run_case core_rs_super_escape fail 'use super::view;' "core.rs:1 breaks 'super_escape'" ../core.rs

# --- attributes split over lines --------------------------------------------

run_case path_in_multiline_cfg_attr fail '#[cfg_attr(
    not(test),
    path = "../../view/mod.rs"
)]
mod v;' "x.rs:1 breaks 'path_attr'"

# --- comments and strings do not open a `mod x {` ---------------------------

run_case mod_open_in_block_comment fail '/* mod pad { */
use super::super::view::Theme;' "x.rs:2 breaks 'super_escape'" x.rs
run_case mod_open_in_string fail 'const S: &str = "mod pad {";
use super::super::view::Theme;' "x.rs:2 breaks 'super_escape'" x.rs
run_case super_word_in_string pass 'const S: &str = "a super user";' '' mod.rs
run_case use_after_block_comment_is_joined fail '/* io */ use std::{
    fs};' "x.rs:1 breaks 'std_fs'"

# --- raw identifiers and char literals --------------------------------------

run_case raw_identifier_path fail 'let _ = std::r#fs::read(p);' "breaks 'std_fs'"
run_case char_quote_does_not_hide_code fail "let q = '\"'; let u = \"http://x\"; let _ = std::fs::read(u);" \
  "breaks 'std_fs'"
run_case lifetimes_are_not_char_literals pass "fn f<'a>(x: &'a str) -> &'a str { x } // std::fs"
run_case non_ascii_char_does_not_hide_code fail "m! {'é','\"'}
use std::fs;
const B: &str = \"x\";" "x.rs:2 breaks 'std_fs'"
run_case spaced_path_attribute fail 'm! { # [path = "../../view/mod.rs"] mod v; }' \
  "x.rs:1 breaks 'path_attr'"

# --- the UI stack -------------------------------------------------------------

run_case wgpu_use fail 'use wgpu::Device;' "breaks 'ui_stack'"
run_case signex_widgets_path fail 'let b = signex_widgets::button(x);' "breaks 'ui_stack'"
run_case i18n_ids_are_allowed pass 'use signex_i18n_ids::NoticeId;'
run_case signex_i18n_use fail 'use signex_i18n::I18n;' "breaks 'ui_stack'"

# --- spaced paths (rustfmt leaves them inside macro_rules! bodies) ----------

run_case spaced_std_path fail 'macro_rules! m { () => { std :: fs :: read("x") }; }' \
  "breaks 'std_fs'"
run_case spaced_crate_path fail 'macro_rules! m { () => { crate :: app :: handlers :: x() }; }' \
  "breaks 'crate_outside_core'"
run_case spaced_super_path fail 'macro_rules! m { () => { super :: super :: handlers :: x() }; }' \
  "x.rs:1 breaks 'super_escape'" x.rs

# --- visibility restrictions are not dependencies ---------------------------

run_case pub_super_in_core_root pass 'pub(super) fn new() {}' '' mod.rs
run_case pub_super_in_core_rs pass 'pub(super) fn new() {}' '' ../core.rs
run_case pub_in_crate_path pass 'pub(in crate::app) fn f() {}
pub(crate) struct S;'
run_case super_escape_next_to_pub_super fail 'pub(super) fn new() {}
use super::super::x;' "mod.rs:2 breaks 'super_escape'" mod.rs

# --- file names are not code ----------------------------------------------------

run_case iced_in_file_name pass 'fn f() {}' '' iced_free.rs
run_case rfd_in_file_name pass 'fn f() {}' '' sub/rfd.rs

# --- the core declared where the guard does not look ------------------------

run_case inline_core_module_in_app_rs fail 'mod core { use std::fs; }' \
  "app.rs:1 breaks 'core_decl'" ../../app.rs
run_case core_module_behind_path_attr fail '#[path = "elsewhere/core.rs"]
#[allow(dead_code)]
mod core;' "app.rs:3 breaks 'core_decl'" ../../app.rs
run_case plain_core_module_in_app_rs pass 'pub mod core;
#[path = "x.rs"]
mod other;' '' ../../app.rs

# --- and the real tree passes ----------------------------------------------

if bash "$GATE" >/dev/null 2>&1; then
  echo "ok    real_tree"
else
  echo "FAIL  real_tree — the gate rejects the current workspace"
  failures=$((failures + 1))
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All core-guard controls passed."
