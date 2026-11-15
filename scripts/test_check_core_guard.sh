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

failures=0

# run_case <name> <fail|pass> <core file body | -none- | -empty-> [message] [file]
#
# `file` is the path below app/core/ (default sub/x.rs, module depth 2).
# The exit code alone is not enough: a gate that crashes under `set -e` also
# exits 1. A fail case must print `message`; a pass case the OK line.
run_case() {
  local name="$1" expect="$2" body="$3" want="${4:-}" file="${5:-sub/x.rs}" tree="$TMP/$1" status=0 out
  mkdir -p "$tree/crates/signex-app/src/app"
  case "$body" in
    -none-) ;;
    -empty-) mkdir -p "$tree/$CORE_REL" ;;
    *) mkdir -p "$(dirname "$tree/$CORE_REL/$file")"; printf '%s\r\n' "$body" > "$tree/$CORE_REL/$file" ;;
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
};' "x.rs:1 breaks 'std_fs': use std::fs;"
run_case multiline_crate_group_inside_core pass 'pub(crate) use crate::{
    app::core::x,
    app::core::{y, z},
};'
run_case multiline_crate_group_escapes fail 'use crate::{
    app::core::x,
    app::other,
};' "breaks 'crate_outside_core': use crate::app::other;"
run_case nested_crate_group_escapes fail 'use crate::app::{core::x, view::Y};' \
  "breaks 'crate_outside_core': use crate::app::view::Y;"
run_case second_statement_on_a_line fail 'use crate::app::core::a; use crate::keymap::K;' \
  "breaks 'crate_outside_core'"

# --- aliasing the crate root ------------------------------------------------

run_case use_crate_as fail 'use crate as root;' "breaks 'crate_alias'"
run_case use_crate_self_as fail 'use crate::{self as root};' "breaks 'crate_alias'"
run_case extern_crate_self_as fail 'extern crate self as root;' "breaks 'crate_alias'"

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
