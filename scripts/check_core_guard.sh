#!/usr/bin/env bash
#
# Application-core guard (ADR-0006, "While the core grows inside
# `signex-app`"). The core grows in crates/signex-app/src/app/core/ until it is
# lifted into its own crate; until then this script keeps it apart. A line in
# any .rs file under that directory, or in the sibling module root
# crates/signex-app/src/app/core.rs (module depth 0), fails when it:
#
#   crate_outside_core  names a `crate::` path outside `crate::app::core`;
#   crate_alias         aliases the crate root (`use crate as x`,
#                       `use crate::{self as x}`, `extern crate self as x`),
#                       which would hide a path from the rule above;
#   std_alias           aliases the `std`, `core` or `alloc` root (`use std as s`,
#                       `use std::{self as s}`, `extern crate std as s`), which
#                       would hide `std::fs` and the rest from the rules below;
#   path_attr           uses `#[path = ...]`, which mounts a file from anywhere;
#   include_source      uses `include!`, which pastes outside source in
#                       (`include_str!` / `include_bytes!` of data are fine);
#   super_escape        climbs out of app/core/ with a `super::` chain, judged
#                       against the file's module depth (`a/b.rs` is 2 deep,
#                       `a/mod.rs` 1) plus the inline `mod x {` blocks around it;
#   iced                names `iced` (or an `iced_*` crate);
#   ui_stack            names `wgpu`, `winit`, `rfd`, `signex_widgets`,
#                       `signex_gfx`, `signex_renderer` or `signex_i18n`
#                       (`signex_i18n_ids` stays allowed: core notices are its
#                       keys);
#   tokio               names `tokio`;
#   blocking            names `spawn_blocking` or `run_blocking`;
#   std_fs / std_thread / std_process
#                       names `std::fs`, `std::thread` or `std::process`.
#
# And the app module root (crates/signex-app/src/app.rs or app/mod.rs) fails
# on core_decl: a `mod core` with an inline `{` body or behind a `#[path]`
# attribute, which would put the core where the scan above never looks.
#
# Each file is lexed first (scripts/check_core_guard.awk): `//` and `/* */`
# comments are dropped, string and char literal contents blanked and the `r#`
# of a raw identifier cut, so neither a comment nor a string can hide code or
# fake a `mod x {`. An attribute is joined over its lines, and each `use`
# statement, also one after `#[attr]` or `{` on its line, is joined and
# flattened to one path per record, so a multi-line form is judged like the
# one-line form. A failure reports the original source line. A missing or
# empty core passes.
#
# Known blind spots:
#   a macro defined outside the core and pulled in by `#[macro_use]` expands unseen;
#   std IO roots not listed above (std::net, std::env, std::io::stdin, std::os::*::fs) pass.
#
# This is a source scan, unlike the crate-graph check in
# scripts/check_arch_guard.sh, which reads `cargo metadata`.
#
# Usage: scripts/check_core_guard.sh   # exit 1 on a violation
#
# CORE_GUARD_ROOT=<dir> checks another tree and CORE_GUARD_AWK=<awk> picks the
# awk (both used by the controls script).
#
set -euo pipefail

SCAN="$(cd "$(dirname "$0")" && pwd)/check_core_guard.awk"
ROOT="${CORE_GUARD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
AWK="${CORE_GUARD_AWK:-awk}"
CORE_REL="crates/signex-app/src/app/core"
APP_ROOTS=(crates/signex-app/src/app.rs crates/signex-app/src/app/mod.rs)
# Read by awk through ENVIRON, which (unlike -v) keeps the backslashes.
export PATH_ATTR='#!?[[:space:]]*\[[[:space:]]*(cfg_attr\(.*,[[:space:]]*)?path[[:space:]]*='

cd "$ROOT"

RULES=(crate_outside_core crate_alias std_alias path_attr include_source super_escape
  iced ui_stack tokio blocking std_fs std_thread std_process core_decl)

pattern() {
  case "$1" in
    crate_alias) printf '%s\n' '\b(use[[:space:]]+crate|crate::self|extern[[:space:]]+crate[[:space:]]+self)[[:space:]]+as\b' ;;
    std_alias) printf '%s\n' '\b(use|extern[[:space:]]+crate)[[:space:]]+(::)?(std|core|alloc)[[:space:]]+as\b|(^|[^:A-Za-z0-9_])(::)?(std|core|alloc)::self[[:space:]]+as\b' ;;
    path_attr) printf '%s\n' "$PATH_ATTR" ;;
    include_source) printf '%s\n' '\binclude[[:space:]]*!' ;;
    iced) printf '%s\n' '\biced(\b|_)' ;;
    ui_stack) printf '%s\n' '\b(wgpu|winit|rfd|signex_widgets|signex_gfx|signex_renderer|signex_i18n)\b' ;;
    tokio) printf '%s\n' '\btokio\b' ;;
    blocking) printf '%s\n' '\b(spawn_blocking|run_blocking)\b' ;;
    std_fs) printf '%s\n' '\bstd::(fs\b|\{[^;]*\bfs\b)' ;;
    std_thread) printf '%s\n' '\bstd::(thread\b|\{[^;]*\bthread\b)' ;;
    std_process) printf '%s\n' '\bstd::(process\b|\{[^;]*\bprocess\b)' ;;
  esac
}

# Every core file: core.rs (depth 0), then core/**/*.rs.
core_files() {
  [[ -f "$CORE_REL.rs" ]] && echo "$CORE_REL.rs"
  [[ -d "$CORE_REL" ]] && find "$CORE_REL" -type f -name '*.rs' | sort
  return 0
}

# scan <lines|super>: run the scanner over every core file, `path:line:text`.
scan() {
  local f rel slashes depth
  while IFS= read -r f; do
    rel="${f#"$CORE_REL"/}"
    slashes="${rel//[^\/]/}"
    depth=$(( ${#slashes} + 1 ))
    [[ "$(basename "$f")" == mod.rs ]] && depth=$(( depth - 1 ))
    [[ "$f" == "$CORE_REL.rs" ]] && depth=0
    "$AWK" -v f="$f" -v depth="$depth" -v mode="$1" -f "$SCAN" "$f"
  done < <(core_files)
}

# A `mod core` in the app module root that is inline or behind `#[path]`
# (also with other attributes between the two).
core_decl() {
  local f
  for f in "${APP_ROOTS[@]}"; do
    [[ -f "$f" ]] || continue
    "$AWK" -v f="$f" -v depth=0 -v mode=lines -f "$SCAN" "$f" | "$AWK" '
      { t = $0; sub(/^[^:]*:[0-9]+:/, "", t); is_path = t ~ ENVIRON["PATH_ATTR"] }
      t ~ /(^|[^A-Za-z0-9_])mod[[:space:]]+core([^A-Za-z0-9_]|$)/ &&
        (t ~ /mod[[:space:]]+core[[:space:]]*\{/ || is_path || after_path) { print }
      { after_path = is_path ? 1 : (t ~ /^[[:space:]]*#/ ? after_path : 0) }'
  done
}

hits() {
  case "$1" in
    core_decl) core_decl ;;
    super_escape) scan super ;;
    crate_outside_core)
      # Keep a record when it still names `crate::` once its in-core paths are dropped.
      scan lines | "$AWK" '{ t = $0; sub(/^[^:]*:[0-9]+:/, "", t)
                          gsub(/crate::app::core([^A-Za-z0-9_]|$)/, "", t)
                          if (t ~ /crate::/) print }' ;;
    *) scan lines | { grep -E "$(pattern "$1")" || true; } ;;
  esac
}

# report <rule> <path:line:matched text>: the original line, plus what was
# matched when that differs (a flattened `use` path, a line without its comment).
report() {
  local path="${2%%:*}" rest="${2#*:}" line text orig
  line="${rest%%:*}"
  text="${rest#*:}"
  orig="$(sed -n "${line}p" "$path" | tr -d '\r')"
  orig="${orig#"${orig%%[![:space:]]*}"}"
  echo "Core guard: $path:$line breaks '$1': $orig"
  text="${text#"${text%%[![:space:]]*}"}"
  [[ "$text" != "$orig" ]] && echo "    matched: $text"
  return 0
}

fail=0
for rule in "${RULES[@]}"; do
  while IFS= read -r hit; do
    [[ -z "$hit" ]] && continue
    report "$rule" "$hit"
    fail=1
  done < <(hits "$rule")
done

if (( fail )); then
  echo "  app/core/ stays free of the UI, async runtimes, blocking IO and the rest of"
  echo "  signex-app (ADR-0006). Return the effect to the shell instead."
  exit 1
fi
if [[ ! -d "$CORE_REL" && ! -f "$CORE_REL.rs" ]]; then
  echo "Core guard: OK — $CORE_REL does not exist yet."
else
  echo "Core guard: OK — $CORE_REL names nothing outside the core."
fi
