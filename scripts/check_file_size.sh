#!/usr/bin/env bash
#
# God-file ratchet gate — keep production Rust source out of the technical-debt
# swamp (ADR-0001: vertical slices, "promote a file to a folder on size").
#
# A file's *production* line count is what matters: everything up to the first
# `#[cfg(test)]` marker. Inline test modules and dedicated test files do not
# count — a large, well-tested module is not a god-file; a large tangle of
# production code is.
#
# The gate is a RATCHET, not a hard cap: files already over the cap are
# grandfathered in `scripts/godfile-baseline.txt` with their current size. CI
# fails only when
#   - a NEW production file crosses the cap (not in the baseline), or
#   - a grandfathered file GROWS beyond its recorded size.
# Shrinking is always allowed; drop a file from the baseline once it is under
# the cap to lock the win in. The baseline only ever gets shorter.
#
# Usage:
#   scripts/check_file_size.sh            # check (CI); exit 1 on a violation
#   scripts/check_file_size.sh --generate # rewrite the baseline from the tree
#
set -euo pipefail

CAP=1000
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASELINE="$ROOT/scripts/godfile-baseline.txt"

cd "$ROOT"

# Production source only: crates/**/src/**.rs. Dedicated test files and in-src
# test modules are exempt (tests/ dirs, tests.rs, *_tests.rs).
list_files() {
  find crates -type f -name '*.rs' -path '*/src/*' \
    -not -path '*/target/*' \
    -not -path '*/tests/*' \
    -not -name 'tests.rs' \
    -not -name '*_tests.rs' \
    | sort
}

# Production line count: lines before the first `#[cfg(test)]`, else the whole
# file. `grep -n -m1` gives the 1-based line of the first match; production
# lines are everything above it.
prod_lines() {
  local f="$1" n
  n=$(grep -n -m1 '#\[cfg(test)\]' "$f" | cut -d: -f1 || true)
  if [[ -n "$n" ]]; then
    echo $((n - 1))
  else
    wc -l < "$f" | tr -d ' '
  fi
}

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# God-file baseline — production src/ .rs files currently over the ${CAP}-line cap."
    echo "# Format: <production_lines> <path>. Managed by scripts/check_file_size.sh."
    echo "# The list only ever gets SHORTER: shrink a file under the cap, then drop its row."
    while IFS= read -r f; do
      n=$(prod_lines "$f")
      (( n > CAP )) && echo "$n $f"
    done < <(list_files) | sort -rn
  } > "$BASELINE"
  echo "Wrote baseline: $BASELINE"
  exit 0
fi

# Recorded baseline size for a path, or empty if not grandfathered. Looked up
# with awk rather than an associative array so the script runs on the bash 3.2
# that ships with macOS as well as the bash 5 on CI.
baseline_size() {
  [[ -f "$BASELINE" ]] || return 0
  awk -v p="$1" '$1 ~ /^[0-9]+$/ && $2 == p { print $1; exit }' "$BASELINE"
}

fail=0
while IFS= read -r f; do
  n=$(prod_lines "$f")
  (( n > CAP )) || continue
  b=$(baseline_size "$f")
  if [[ -n "$b" ]]; then
    if (( n > b )); then
      echo "FAIL  $f — grew to $n production lines (baseline $b, cap $CAP)."
      echo "      A grandfathered god-file must shrink, never grow. Split it into"
      echo "      vertical slices (ADR-0001) or move code into a domain crate."
      fail=1
    fi
  else
    echo "FAIL  $f — $n production lines (> $CAP cap) and not grandfathered."
    echo "      Do not add a new god-file. Promote it to a folder of vertical"
    echo "      slices (ADR-0001) — messages / state / updates / views per concern."
    fail=1
  fi
done < <(list_files)

# Advisory: nudge trimming the baseline as files shrink under the cap. Never
# fatal — it only asks the win be recorded so the file can't silently regrow.
while read -r bn bpath; do
  [[ -z "${bn:-}" || "$bn" == \#* ]] && continue
  if [[ ! -f "$bpath" ]]; then
    echo "NOTE  $bpath — in the baseline but gone; remove its row from ${BASELINE#"$ROOT"/}."
  else
    n=$(prod_lines "$bpath")
    (( n <= CAP )) && echo "NOTE  $bpath — now $n lines, under the $CAP cap. Remove its row from ${BASELINE#"$ROOT"/} to lock the win."
  fi
done < "$BASELINE"

if (( fail )); then
  echo ""
  echo "God-file gate failed. See ADR-0001 (docs/architecture) for the slice model."
  exit 1
fi
echo "God-file gate: OK — no new god-files, no grandfathered file grew."
