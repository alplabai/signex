#!/usr/bin/env bash
#
# Application-core guard (ADR-0006, "While the core grows inside
# `signex-app`"). The core grows in crates/signex-app/src/app/core/ until it is
# lifted into its own crate; until then this script keeps it apart. A line in
# any .rs file under that directory fails when it:
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
#   tokio               names `tokio`;
#   blocking            names `spawn_blocking` or `run_blocking`;
#   std_fs / std_thread / std_process
#                       names `std::fs`, `std::thread` or `std::process`.
#
# Each `use` statement, also one after `#[attr]` or `{` on its line, is joined
# over its lines and flattened to one path per record first
# (scripts/check_core_guard.awk), so a multi-line group is judged like the
# one-line form. `//` comments are cut off before matching (outside "..."
# strings); a failure reports the original source line. A missing or empty
# core directory passes. Block comments and string literals are still
# scanned: a hit there fails, the safe side.
#
# This is a source scan, unlike the crate-graph check in
# scripts/check_arch_guard.sh, which reads `cargo metadata`.
#
# Usage: scripts/check_core_guard.sh   # exit 1 on a violation
#
# CORE_GUARD_ROOT=<dir> checks another tree (used by the controls script).
#
set -euo pipefail

SCAN="$(cd "$(dirname "$0")" && pwd)/check_core_guard.awk"
ROOT="${CORE_GUARD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
CORE_REL="crates/signex-app/src/app/core"

if [[ ! -d "$ROOT/$CORE_REL" ]]; then
  echo "Core guard: OK — $CORE_REL does not exist yet."
  exit 0
fi

cd "$ROOT"

RULES=(crate_outside_core crate_alias std_alias path_attr include_source super_escape
  iced tokio blocking std_fs std_thread std_process)

pattern() {
  case "$1" in
    crate_alias) printf '%s\n' '\b(use[[:space:]]+crate|crate::self|extern[[:space:]]+crate[[:space:]]+self)[[:space:]]+as\b' ;;
    std_alias) printf '%s\n' '\b(use|extern[[:space:]]+crate)[[:space:]]+(::)?(std|core|alloc)[[:space:]]+as\b|(^|[^:A-Za-z0-9_])(::)?(std|core|alloc)::self[[:space:]]+as\b' ;;
    path_attr) printf '%s\n' '#!?\[.*\bpath[[:space:]]*=' ;;
    include_source) printf '%s\n' '\binclude[[:space:]]*!' ;;
    iced) printf '%s\n' '\biced(\b|_)' ;;
    tokio) printf '%s\n' '\btokio\b' ;;
    blocking) printf '%s\n' '\b(spawn_blocking|run_blocking)\b' ;;
    std_fs) printf '%s\n' '\bstd::(fs\b|\{[^;]*\bfs\b)' ;;
    std_thread) printf '%s\n' '\bstd::(thread\b|\{[^;]*\bthread\b)' ;;
    std_process) printf '%s\n' '\bstd::(process\b|\{[^;]*\bprocess\b)' ;;
  esac
}

# scan <lines|super>: run the scanner over every core file, `path:line:text`.
scan() {
  local f rel slashes depth
  while IFS= read -r f; do
    rel="${f#"$CORE_REL"/}"
    slashes="${rel//[^\/]/}"
    depth=$(( ${#slashes} + 1 ))
    [[ "$(basename "$f")" == mod.rs ]] && depth=$(( depth - 1 ))
    awk -v f="$f" -v depth="$depth" -v mode="$1" -f "$SCAN" "$f"
  done < <(find "$CORE_REL" -type f -name '*.rs' | sort)
}

hits() {
  case "$1" in
    super_escape) scan super ;;
    crate_outside_core)
      # Keep a record when it still names `crate::` once its in-core paths are dropped.
      scan lines | awk '{ t = $0; sub(/^[^:]*:[0-9]+:/, "", t)
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
echo "Core guard: OK — $CORE_REL names nothing outside the core."
