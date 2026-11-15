#!/usr/bin/env bash
#
# Application-core guard (ADR-0006, "While the core grows inside
# `signex-app`"). The core grows in crates/signex-app/src/app/core/ until it is
# lifted into its own crate; until then this script keeps it apart. A line in
# any .rs file under that directory fails when it:
#
#   crate_outside_core  names a `crate::` path outside `crate::app::core`;
#   iced                names `iced` (or an `iced_*` crate);
#   tokio               names `tokio`;
#   blocking            names `spawn_blocking` or `run_blocking`;
#   std_fs / std_thread / std_process
#                       names `std::fs`, `std::thread` or `std::process`
#                       (also inside a one-line `use std::{...}` group).
#
# A missing or empty core directory passes. Lines whose first non-blank text
# is `//` (comments, doc comments) are skipped. Blind spots: block comments
# and string literals are scanned (a hit there fails, the safe side); a
# `use std::{` group split over several lines, `super::` paths that climb out
# of the core, and `use crate as x` aliases are not seen.
#
# This is a source scan, unlike the crate-graph check in
# scripts/check_arch_guard.sh, which reads `cargo metadata`.
#
# Usage: scripts/check_core_guard.sh   # exit 1 on a violation
#
# CORE_GUARD_ROOT=<dir> checks another tree (used by the controls script).
#
set -euo pipefail

ROOT="${CORE_GUARD_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
CORE_REL="crates/signex-app/src/app/core"
CORE="$ROOT/$CORE_REL"

if [[ ! -d "$CORE" ]]; then
  echo "Core guard: OK — $CORE_REL does not exist yet."
  exit 0
fi

cd "$ROOT"

RULES=(crate_outside_core iced tokio blocking std_fs std_thread std_process)

pattern() {
  case "$1" in
    crate_outside_core) printf '%s\n' '\bcrate::' ;;
    iced) printf '%s\n' '\biced(\b|_)' ;;
    tokio) printf '%s\n' '\btokio\b' ;;
    blocking) printf '%s\n' '\b(spawn_blocking|run_blocking)\b' ;;
    std_fs) printf '%s\n' '\bstd::(fs\b|\{[^}]*\bfs\b)' ;;
    std_thread) printf '%s\n' '\bstd::(thread\b|\{[^}]*\bthread\b)' ;;
    std_process) printf '%s\n' '\bstd::(process\b|\{[^}]*\bprocess\b)' ;;
  esac
}

# Hits for one rule as `path:line:text`. The /dev/null operand keeps grep
# printing file names; tr strips CRs so Git Bash agrees with CI.
hits() {
  find "$CORE_REL" -type f -name '*.rs' | sort \
    | { xargs grep -nE "$(pattern "$1")" /dev/null || true; } | tr -d '\r' \
    | { grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true; } \
    | if [[ "$1" == crate_outside_core ]]; then
        # Keep a line when it still names `crate::` once its in-core paths are dropped.
        awk '{ t = $0; sub(/^[^:]*:[0-9]+:/, "", t)
               gsub(/crate::app::core([^A-Za-z0-9_]|$)/, "", t)
               if (t ~ /crate::/) print }'
      else
        cat
      fi
}

fail=0
for rule in "${RULES[@]}"; do
  while IFS= read -r hit; do
    [[ -z "$hit" ]] && continue
    echo "Core guard: ${hit%%:*}:$(cut -d: -f2 <<<"$hit") breaks '$rule': $(cut -d: -f3- <<<"$hit")"
    fail=1
  done < <(hits "$rule")
done

if (( fail )); then
  echo "  app/core/ stays free of the UI, async runtimes, blocking IO and the rest of"
  echo "  signex-app (ADR-0006). Return the effect to the shell instead."
  exit 1
fi
echo "Core guard: OK — $CORE_REL names nothing outside the core."
