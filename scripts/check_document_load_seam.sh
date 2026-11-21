#!/usr/bin/env bash
#
# Load seam guard (#2326 slice 3, PR 3.B; the wrapper exception is issue
# #2704, ADR-0006). The core half of the schematic load path lives in
# crates/signex-app/src/app/state/document_load/ until slice 4a lifts it. Same
# shape as scripts/check_sheet_change_seam.sh.
#
#   callers   The functions of that module visible outside it (any `pub` or
#             `pub(...)` fn but `pub(self)`) are called only from inside the
#             wrapper functions listed in scripts/document-load-wrappers.txt,
#             and by the module itself. Any use of such a name counts: a call,
#             a path used as a value, an import, or a token inside a macro.
#             The list may only shrink: it holds exactly MAX_WRAPPERS lines and
#             its sorted entries must match WRAPPERS_CKSUM, so adding a wrapper
#             or swapping one name for another needs a visible edit here. A
#             listed wrapper that no longer calls the core half directly fails
#             until its line is removed and both values are lowered with it.
#   gui_names The module names no GUI state, not the diagnostics log, not
#             `catch_unwind`, and none of the helpers that log directly
#             (`log_sheet_repairs`, `report_bare_pin_taps`,
#             `report_unrecognized_sheet_size`, `warn_if_read_only`). A line
#             for Messages is a Notice.
#   no_traits The module declares no `trait` and no `impl .. for ..`.
#   closures  No closure or fn-pointer type (`impl FnOnce(..)`, `F: Fn(..)`,
#             `fn(..)`): wrapper code never runs inside the core half.
#   removal   `<map>.engines.remove(..)` appears exactly once, inside an
#             `Err(..)` match arm: a failed install behind a tab drops the
#             stale engine (#2326 D5). Every other engine goes through the
#             existing DocumentState helpers.
#
# All read the code through ast-grep, so comments do not count. Test code is
# left out: files named `tests.rs` or `*_tests.rs`, anything under a `tests/`
# or `*_tests/` folder, and items under `#[cfg(test)]`.
#
# Blind spots: those of scripts/check_sheet_change_seam.sh (a renamed import is
# caught at the import only; a macro-generated method is not guarded; a second
# fn of a listed name in that file counts as the wrapper; a local binding named
# like a core function counts as a use; a production file named like a test is
# skipped). The removal rule counts the literal `.engines.remove` only: a
# helper that removes inside (`clear_active_engine`) is not counted.
#
# Usage: scripts/check_document_load_seam.sh   (DOCUMENT_LOAD_SEAM_ROOT
# overrides the tree, for scripts/test_check_document_load_seam.sh)
set -euo pipefail

# The size of scripts/document-load-wrappers.txt and the `cksum` of its sorted
# entries (#2704). Lower both when a wrapper goes; never raise or swap.
MAX_WRAPPERS="${DOCUMENT_LOAD_SEAM_MAX:-7}"
WRAPPERS_CKSUM="${DOCUMENT_LOAD_SEAM_CKSUM:-2404267466}"

ROOT="${DOCUMENT_LOAD_SEAM_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
RULES="$(cd "$(dirname "$0")" && pwd)/document-load-seam.yml"
LIST="scripts/document-load-wrappers.txt"
APP="crates/signex-app/src/app"
CORE="$APP/state/document_load"

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Load seam guard: FAIL — $tool is not installed."
    exit 2
  fi
done
if [[ ! -d "$CORE" || ! -f "$LIST" ]]; then
  echo "Load seam guard: FAIL — no $CORE or $LIST under $ROOT."
  exit 2
fi

# Matches outside test files (jq filter on ast-grep JSON).
PRODUCTION='.[]
  | select(.file | test("(^|/|_)tests/|(^|/|_)tests\\.rs$") | not)'
NOT_IN_TEST='
  not:
    inside:
      stopBy: end
      any:
        - kind: mod_item
        - kind: function_item
        - kind: impl_item
      follows:
        stopBy:
          not:
            any:
              - kind: attribute_item
              - kind: line_comment
              - kind: block_comment
        pattern: "#[cfg(test)]"'
FN_NAME='capture("fn\\s+(?<n>[A-Za-z_][A-Za-z0-9_]*)").n'

fail=0
listed="$(grep -vE '^[[:space:]]*(#|$)' "$LIST" | tr -s ' \t' ' ' | sed 's/^ //; s/ $//' | sort -u)"
count="$(printf '%s\n' "$listed" | grep -c . || true)"
if (( count > MAX_WRAPPERS )); then
  echo "Load seam guard: FAIL — $LIST lists $count wrappers, at most $MAX_WRAPPERS (#2704)."
  echo "  The list may only shrink: call an existing wrapper instead of adding one."
  fail=1
elif (( count < MAX_WRAPPERS )); then
  echo "Load seam guard: FAIL — $LIST lists $count wrappers; lower MAX_WRAPPERS in $0 to $count."
  fail=1
fi
cksum_now="$(printf '%s\n' "$listed" | cksum | cut -d' ' -f1)"
if [[ "$cksum_now" != "$WRAPPERS_CKSUM" ]]; then
  echo "Load seam guard: FAIL — the names in $LIST changed (cksum $cksum_now, expected $WRAPPERS_CKSUM)."
  echo "  A wrapper may be removed, never added or swapped (#2704); after a removal set WRAPPERS_CKSUM in $0."
  fail=1
fi

# The fn names of the core module visible outside it.
if ! decls="$(ast-grep scan --json=compact --inline-rules "
id: document-load-visible-fn
language: rust
rule:
  kind: function_item
  has:
    kind: visibility_modifier
    regex: '^pub'
    not:
      regex: '^pub\\(\\s*self\\s*\\)\$'
$NOT_IN_TEST" "$CORE")"; then
  echo "Load seam guard: FAIL — ast-grep could not list $CORE's functions."
  exit 2
fi
names="$(jq -r "$PRODUCTION | .text | $FN_NAME" <<<"$decls" | sort -u | paste -sd '|' -)"

if [[ -n "$names" ]]; then
  USE="any:
      - {kind: call_expression, has: {field: function, regex: '(^|[.:])($names)\$'}}
      - {kind: identifier, regex: '^($names)\$'}"
  if ! uses="$(ast-grep scan --json=compact --inline-rules "
id: document-load-use
language: rust
rule:
  all:
    - $USE
  $NOT_IN_TEST" crates)" || ! callers="$(ast-grep scan --json=compact --inline-rules "
id: document-load-caller
language: rust
rule:
  kind: function_item
  has:
    stopBy: end
    $USE" crates)"; then
    echo "Load seam guard: FAIL — ast-grep could not scan the calls."
    exit 2
  fi
  # Listed wrappers as {file, name, start, end} line ranges.
  allowed="$(jq -c --arg app "$APP/" --arg listed "$listed" "[$PRODUCTION
    | {file: (.file | ltrimstr(\$app)), name: (.text | $FN_NAME),
       start: (.range.start.line + 1), end: (.range.end.line + 1)}
    | select((\"\(.file) \(.name)\") as \$k | \$listed | split(\"\n\") | index(\$k))]" <<<"$callers")"
  outside="$(jq -r --arg core "$CORE/" --arg app "$APP/" --argjson allowed "$allowed" "$PRODUCTION
    | select(.file | startswith(\$core) | not)
    | (.file | ltrimstr(\$app)) as \$f | (.range.start.line + 1) as \$l
    | select([\$allowed[] | select(.file == \$f and .start <= \$l and \$l <= .end)] | length == 0)
    | \"\(.file):\(\$l): \(.text | split(\"\n\")[0])\"" <<<"$uses" | sort -u)"
  if [[ -n "$outside" ]]; then
    echo "Load seam guard: FAIL — the core half of the load path is called outside its wrappers (#2704):"
    printf '%s\n' "$outside" | awk '{ print "  " $0 }'
    echo "  Call a listed wrapper (open_schematic_tab, apply_loaded_schematic, ...) instead."
    fail=1
  fi
  live="$(jq -r '.[] | "\(.file) \(.name)"' <<<"$allowed" | sort -u)"
  stale="$(comm -23 <(printf '%s\n' "$listed") <(printf '%s\n' "$live") | sed '/^$/d')"
  if [[ -n "$stale" ]]; then
    echo "Load seam guard: FAIL — listed wrappers no longer call the core half; remove their lines from $LIST:"
    printf '%s\n' "$stale" | awk '{ print "  " $0 }'
    fail=1
  fi
fi

if ! named="$(ast-grep scan --rule "$RULES" --json=compact "$CORE")"; then
  echo "Load seam guard: FAIL — ast-grep could not run $RULES."
  exit 2
fi
hits() {
  jq -r "$PRODUCTION"' | select(.ruleId == "'"$1"'")
    | "\(.file):\(.range.start.line + 1): \(.text | split("\n")[0])"' <<<"$named" | sort
}
gui="$(hits document-load-gui-name)"
traits="$(hits document-load-trait)"
closures="$(hits document-load-closure)"
removals="$(hits document-load-engines-remove)"
in_err="$(hits document-load-engines-remove-err)"
if [[ -n "$traits" ]]; then
  echo "Load seam guard: FAIL — $CORE declares a trait or a trait impl:"
  printf '%s\n' "$traits" | awk '{ print "  " $0 }'
  echo "  A trait method would reach the core half past the wrapper list (#2704); use an inherent fn."
  fail=1
fi
if [[ -n "$gui" ]]; then
  echo "Load seam guard: FAIL — $CORE names GUI state, the diagnostics log, catch_unwind or a logging helper:"
  printf '%s\n' "$gui" | awk '{ print "  " $0 }'
  echo "  Return a Notice and let the wrapper log it through app/projector (#2326 M6)."
  fail=1
fi
if [[ -n "$closures" ]]; then
  echo "Load seam guard: FAIL — $CORE takes a closure or fn-pointer type:"
  printf '%s\n' "$closures" | awk '{ print "  " $0 }'
  echo "  The wrapper runs its GUI steps around the core half, never inside it."
  fail=1
fi
n_removals="$(printf '%s' "$removals" | grep -c . || true)"
n_in_err="$(printf '%s' "$in_err" | grep -c . || true)"
if (( n_removals != 1 || n_in_err != 1 )); then
  echo "Load seam guard: FAIL — $CORE must remove from the engine map exactly once, in the Err arm of a failed install behind a tab (found $n_removals, $n_in_err in an Err arm):"
  printf '%s\n' "$removals" | sed '/^$/d' | awk '{ print "  " $0 }'
  fail=1
fi

if (( fail )); then
  exit 1
fi
echo "Load seam guard: OK — the core half is called only by the $count listed wrappers (#2704); no GUI names, traits or closures in $CORE; one engine removal, in its Err arm."
