#!/usr/bin/env bash
#
# Engine id-minting ratchet (#2429, ADR-0006): the engine is deterministic, so
# a new object id comes from the caller (`signex_types::object_id::IdSource`),
# never from `Uuid::now_v7` / `Uuid::new_v4` inside crates/signex-engine/src.
# The sites not moved yet are listed in scripts/engine-ids-allowlist.txt as
# `<path under src>:<enclosing fn> <uses>`. The gate fails on a hit that is
# not listed and on a listed entry with no hit left, so the list only shrinks.
#
# A listed fn is a minting helper: every other fn that calls it (or passes it
# as a fn pointer) mints through it. `<uses>` is how many times the engine's
# production code names the helper outside its own `fn` line; the gate fails
# when it grows (a new caller, as the pin-dot paths once were through
# `junction_at`) and when it falls (lower the number), so it only shrinks too.
# A missing `<uses>` is 0.
#
# Test code is skipped: files named `tests.rs`, `*_tests.rs`, `test_support.rs`
# and everything from a trailing `#[cfg(test)] mod <name> {` to EOF (the
# cutoff check_file_size.sh uses). Comment-only lines are skipped.
# Blind spots: a renamed import (`use uuid::Uuid as Id`, `use x::helper as y`),
# a mid-file test module, a hit whose enclosing `fn` line is a multi-line
# signature split before the name, and a caller of a caller of a helper (one
# level is counted).
#
# Usage: scripts/check_engine_ids.sh   (ENGINE_IDS_ROOT overrides the tree,
# for scripts/test_check_engine_ids.sh)
set -euo pipefail

ROOT="${ENGINE_IDS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
SRC="crates/signex-engine/src"
ALLOWLIST="scripts/engine-ids-allowlist.txt"

cd "$ROOT"
if [[ ! -d "$SRC" ]]; then
  echo "Engine id gate: FAIL — no $SRC under $ROOT."
  exit 2
fi

list_files() {
  find "$SRC" -type f -name '*.rs' \
    -not -name 'tests.rs' \
    -not -name '*_tests.rs' \
    -not -name 'test_support.rs' \
    | sort
}

# Prints `<fn name>` once per minting call in the production part of `$1`.
minting_fns() {
  awk '
    /^#\[cfg\(test\)\][[:space:]]*$/ { pending = NR; next }
    pending {
      if ($0 ~ /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\{[[:space:]]*$/) exit
      pending = 0
    }
    {
      if (match($0, /(^|[^A-Za-z0-9_])fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)) {
        name = substr($0, RSTART, RLENGTH)
        sub(/^.*fn[[:space:]]+/, "", name)
        current = name
      }
      if ($0 ~ /^[[:space:]]*\/\//) next
      if ($0 ~ /Uuid::(now_v7|new_v4)/) print (current == "" ? "<top level>" : current)
    }
  ' "$1"
}

allowed() {
  [[ -f "$ALLOWLIST" ]] || return 0
  grep -vE '^[[:space:]]*(#|$)' "$ALLOWLIST" || true
}

# Prints how many times the production part of `$2` names the fn `$1` (a call
# or a fn pointer), its own `fn $1` line and comment lines aside.
# Prints `<fn> <uses>` for each fn named in `$1` (space-separated): how many
# times the production part of the engine names it (a call or a fn pointer),
# its own `fn <name>` line and comment lines aside. One pass over the tree.
name_uses() {
  local names="$1" files
  files="$(mktemp)"
  list_files > "$files"
  # The file list is read inside awk, so no command line grows with the tree.
  awk -v names="$names" -v list="$files" '
    BEGIN {
      count = split(names, ns, " ")
      for (i = 1; i <= count; i++) n[ns[i]] = 0
      while ((getline f < list) > 0) ARGV[ARGC++] = f
      close(list)
    }
    FNR == 1 { skip = 0; pending = 0 }
    skip { next }
    /^#\[cfg\(test\)\][[:space:]]*$/ { pending = FNR; next }
    pending {
      if ($0 ~ /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\{[[:space:]]*$/) { skip = 1; next }
      pending = 0
    }
    {
      if ($0 ~ /^[[:space:]]*\/\//) next
      for (i = 1; i <= count; i++) {
        name = ns[i]
        if ($0 ~ ("(^|[^A-Za-z0-9_])fn[[:space:]]+" name "([^A-Za-z0-9_]|$)")) continue
        line = $0
        word = "(^|[^A-Za-z0-9_])" name "([^A-Za-z0-9_]|$)"
        while (match(line, word)) { n[name]++; line = substr(line, RSTART + RLENGTH) }
      }
    }
    END { for (i = 1; i <= count; i++) print ns[i], n[ns[i]] }
  '
  rm -f "$files"
}

# Prints `<entry> <listed uses> <found uses>` for every listed helper whose
# use count differs from the one written beside it.
use_drift() {
  local names
  names="$(allowed | awk '{ sub(/^.*:/, "", $1); print $1 }' | sort -u | tr '\n' ' ')"
  [[ -z "${names// /}" ]] && return 0
  local uses
  uses="$(name_uses "$names")"
  allowed | awk -v uses="$uses" '
    BEGIN {
      lines = split(uses, rows, "\n")
      for (i = 1; i <= lines; i++) { split(rows[i], kv, " "); found[kv[1]] = kv[2] }
    }
    NF >= 1 {
      name = $1; sub(/^.*:/, "", name)
      listed = (NF >= 2 ? $2 : 0)
      if (listed + 0 != found[name] + 0) print $1, listed, found[name] + 0
    }
  '
}

found="$(
  while IFS= read -r f; do
    while IFS= read -r fn_name; do
      [[ -n "$fn_name" ]] && echo "${f#"$SRC"/}:$fn_name"
    done < <(minting_fns "$f")
  done < <(list_files) | sort -u
)"
listed="$(allowed | awk '{ print $1 }' | sort -u)"
drift="$(use_drift)"

new_sites="$(comm -23 <(printf '%s\n' "$found" | grep -v '^$' || true) <(printf '%s\n' "$listed" | grep -v '^$' || true))"
stale="$(comm -13 <(printf '%s\n' "$found" | grep -v '^$' || true) <(printf '%s\n' "$listed" | grep -v '^$' || true))"

status=0
if [[ -n "$new_sites" ]]; then
  echo "Engine id gate: FAIL — the engine mints ids here; take them from the caller's IdSource:"
  printf '%s\n' "$new_sites" | awk '{ print "  " $0 }'
  status=1
fi
if [[ -n "$stale" ]]; then
  echo "Engine id gate: FAIL — these allowlist entries no longer mint; delete them from $ALLOWLIST:"
  printf '%s\n' "$stale" | awk '{ print "  " $0 }'
  status=1
fi
grown="$(printf '%s\n' "$drift" | awk 'NF == 3 && $3 > $2')"
shrunk="$(printf '%s\n' "$drift" | awk 'NF == 3 && $3 < $2')"
if [[ -n "$grown" ]]; then
  echo "Engine id gate: FAIL — a minting helper has new callers; take the ids from the caller's IdSource (entry, listed uses, found):"
  printf '%s\n' "$grown" | awk '{ print "  " $0 }'
  status=1
fi
if [[ -n "$shrunk" ]]; then
  echo "Engine id gate: FAIL — a minting helper has fewer callers; lower its count in $ALLOWLIST (entry, listed uses, found):"
  printf '%s\n' "$shrunk" | awk '{ print "  " $0 }'
  status=1
fi
if [[ $status -eq 0 ]]; then
  echo "Engine id gate: OK — engine minting only at the $(printf '%s\n' "$listed" | grep -vc '^$' || true) allowlisted sites."
fi
exit $status
