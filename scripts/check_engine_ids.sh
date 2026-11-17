#!/usr/bin/env bash
#
# Engine id-minting ratchet (#2429, ADR-0006): the engine is deterministic, so
# a new object id comes from the caller (`signex_types::object_id::IdSource`),
# never from `Uuid::now_v7` / `Uuid::new_v4` inside crates/signex-engine/src.
# The sites not moved yet are listed in scripts/engine-ids-allowlist.txt as
# `<path under src>:<enclosing fn>`. The gate fails on a hit that is not
# listed and on a listed entry with no hit left, so the list only shrinks.
#
# Test code is skipped: files named `tests.rs`, `*_tests.rs`, `test_support.rs`
# and everything from a trailing `#[cfg(test)] mod <name> {` to EOF (the
# cutoff check_file_size.sh uses). Comment-only lines are skipped.
# Blind spots: a renamed import (`use uuid::Uuid as Id`), a mid-file test
# module, and a hit whose enclosing `fn` line is a multi-line signature
# split before the name.
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

found="$(
  while IFS= read -r f; do
    while IFS= read -r fn_name; do
      [[ -n "$fn_name" ]] && echo "${f#"$SRC"/}:$fn_name"
    done < <(minting_fns "$f")
  done < <(list_files) | sort -u
)"
listed="$(allowed | sort -u)"

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
if [[ $status -eq 0 ]]; then
  echo "Engine id gate: OK — engine minting only at the $(printf '%s\n' "$listed" | grep -vc '^$' || true) allowlisted sites."
fi
exit $status
