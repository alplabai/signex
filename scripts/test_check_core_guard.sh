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

# run_case <name> <fail|pass> <core file body | -none- | -empty-> [message]
#
# The exit code alone is not enough: a gate that crashes under `set -e` also
# exits 1. A fail case must print `message`; a pass case the OK line.
run_case() {
  local name="$1" expect="$2" body="$3" want="${4:-}" tree="$TMP/$1" status=0 out
  mkdir -p "$tree/crates/signex-app/src/app"
  case "$body" in
    -none-) ;;
    -empty-) mkdir -p "$tree/$CORE_REL" ;;
    *) mkdir -p "$tree/$CORE_REL/sub"; printf '%s\r\n' "$body" > "$tree/$CORE_REL/sub/x.rs" ;;
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
