#!/usr/bin/env bash
#
# Legacy-English ratchet (#2326 decision M2, #2644). App-authored English
# that has no catalog id yet reaches a command outcome or a notice through
# named constructors:
#
#   command_outcome_disabled     calls of `CommandOutcome::disabled(..)`
#   translatable_legacy_english  calls of `Translatable::legacy_english(..)`
#   notice_legacy_english        calls of `Notice::legacy_english(..)`, the
#                                edit tails' core notices (#2326 slice 2),
#                                which wrap the one above
#
# Each count, over production code under crates/ (files named `tests.rs`,
# `tests_*.rs`, `*_tests.rs` or under a `tests/` folder are left out), is
# compared with scripts/legacy-english-baseline.txt. A count above its
# baseline fails (new English that bypasses the catalog); a count below it
# fails too, so the baseline is lowered in the PR that pays the debt.
# Calls are found with ast-grep (scripts/legacy-english-call.yml), so a
# qualified path counts and a comment does not.
#
# Blind spots: an inline `#[cfg(test)] mod tests` in a production file is
# counted; a call through an area's own wrapper (`refused(reason)`) counts
# once, at the wrapper.
#
# Usage:
#   scripts/check_legacy_english.sh              # check (CI); exit 1 on drift
#   scripts/check_legacy_english.sh --generate   # rewrite the baseline
#
# LEGACY_ENGLISH_ROOT=<dir> measures another tree (used by the controls).
set -euo pipefail

SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
ROOT="${LEGACY_ENGLISH_ROOT:-$(cd "$SCRIPTS/.." && pwd)}"
RULE="$SCRIPTS/legacy-english-call.yml"
BASELINE="$ROOT/scripts/legacy-english-baseline.txt"
KEYS=(command_outcome_disabled translatable_legacy_english notice_legacy_english)

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null; then
    echo "Legacy-English ratchet: FAIL — $tool is not installed."
    exit 2
  fi
done

if ! scan="$(ast-grep scan --rule "$RULE" --json=stream crates)"; then
  echo "Legacy-English ratchet: FAIL — ast-grep could not run $RULE."
  exit 2
fi

# `rule-id path:line` of every production call.
hits() {
  jq -r '"\(.ruleId) \(.file):\(.range.start.line + 1)"' <<<"$scan" |
    { grep -Ev '(^|/)tests(_[A-Za-z0-9_]+)?\.rs:|_tests\.rs:|/tests/' || true; } |
    tr -d '\r'
}

rule_of() {
  case "$1" in
    command_outcome_disabled) echo command-outcome-disabled ;;
    translatable_legacy_english) echo translatable-legacy-english ;;
    notice_legacy_english) echo notice-legacy-english ;;
  esac
}

count() { hits | { grep -c "^$(rule_of "$1") " || true; }; }

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# Legacy-English ratchet baseline (scripts/check_legacy_english.sh). Counts may only go down."
    echo "# Regenerate with: bash scripts/check_legacy_english.sh --generate"
    for k in "${KEYS[@]}"; do echo "$k=$(count "$k")"; done
  } > "$BASELINE.tmp"
  mv "$BASELINE.tmp" "$BASELINE"
  echo "Wrote ${BASELINE#"$ROOT"/}"
  exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
  echo "Legacy-English ratchet: ${BASELINE#"$ROOT"/} is missing; run with --generate."
  exit 1
fi

fail=0
for k in "${KEYS[@]}"; do
  base="$({ grep -E "^$k=" "$BASELINE" || true; } | tail -n 1 | cut -d= -f2 | tr -d '\r ')"
  if ! [[ "$base" =~ ^[0-9]+$ ]]; then
    echo "Legacy-English ratchet: no numeric baseline for '$k' in ${BASELINE#"$ROOT"/}."
    fail=1
    continue
  fi
  now="$(count "$k")"
  if (( now > base )); then
    echo "Legacy-English ratchet: '$k' grew from $base to $now. Calls:"
    hits | grep "^$(rule_of "$k") " | sed 's/^[^ ]* /    /'
    echo "  Give the text a signex-i18n-ids catalog id instead (#2644)."
    fail=1
  elif (( now < base )); then
    echo "Legacy-English ratchet: '$k' fell from $base to $now. Lock in the gain:"
    echo "  bash scripts/check_legacy_english.sh --generate, then commit the baseline."
    fail=1
  fi
done

if (( fail )); then
  exit 1
fi
echo "Legacy-English ratchet: OK — counts match scripts/legacy-english-baseline.txt."
