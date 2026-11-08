#!/usr/bin/env bash
#
# Raw-label ratchet (#2056, i18n epic #2053). The house widgets in
# signex-widgets take a `Localized`, a `UserText` or `raw("…")`, and clippy
# bans the iced functions they wrap. `raw` is the one escape hatch for text
# that is not in the message catalog yet, so this script counts its call sites
# in the UI crates (signex-app, signex-widgets, chrome-catalog) and compares
# the count with the checked-in scripts/raw-labels-baseline.txt. The count must
# equal the baseline: a higher count fails, and so does a lower one until the
# same change lowers the baseline (`--generate`), so a migration cannot leave
# headroom for new raw labels. Two migration PRs that both lower the number
# conflict on the baseline file; the second regenerates it after its merge.
#
# A second check (scripts/raw_label_macro_literals.pl) fails on a bare string
# literal that is an element of `row![…]`, `column![…]` or `stack![…]`: `&str`
# is a widget in this iced fork, so such a literal skips `text(raw(…))`.
# ast-grep rules cover the rest (no-raw-literal-widget, no-raw-alias).
#
# Known limits: the count is per call site, so a helper that wraps `raw` once
# and is called many times counts once (crates/chrome-catalog/src/bom_modal.rs
# does this); an alias or `raw` passed as a value is refused by the ast-grep
# rule no-raw-alias, except `f(raw)` on a plain function; a raw string
# `r#"…"#` inside a row!/column! element escapes the macro check.
#
# The count is textual, not an ast-grep query: iced's `row![…]`/`column![…]`
# macros hold their arguments as unparsed token trees, which an AST search
# does not see, and about half of the `text(raw(…))` calls sit inside them.
# What counts: `raw(` not preceded by an identifier character, `.` or `::`,
# plus the paths `signex_widgets::raw(` and `crate::raw(`; comment lines, `fn
# raw(` definitions, `tests/` directories and the module that defines `raw`
# (signex-widgets/src/label.rs) are skipped.
#
# Usage:
#   scripts/check_raw_labels.sh              # check (CI); exit 1 on growth
#   scripts/check_raw_labels.sh --generate   # rewrite the baseline from the tree
#
# RAW_LABELS_ROOT=<dir> measures another tree (used by the controls script).
#
set -euo pipefail

ROOT="${RAW_LABELS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
BASELINE="$ROOT/scripts/raw-labels-baseline.txt"
KEY=raw_labels
CALL='(^|[^[:alnum:]_.:])raw\(|signex_widgets::raw\(|crate::raw\('

cd "$ROOT"

# UI-crate Rust sources, minus the module that defines `raw`.
list_files() {
  local dirs=()
  for d in crates/signex-app crates/signex-widgets crates/chrome-catalog; do
    [[ -d "$d" ]] && dirs+=("$d")
  done
  (( ${#dirs[@]} )) || return 0
  find "${dirs[@]}" -type f -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' \
    -not -path 'crates/signex-widgets/src/label.rs' | sort
}

# Call sites, one per line. The /dev/null operand keeps grep printing file
# names; tr strips CRs so Git Bash agrees with CI.
hits() {
  { list_files | xargs grep -nE "$CALL" /dev/null || true; } | tr -d '\r' \
    | { grep -vE '^[^:]+:[0-9]+:[[:space:]]*//|fn[[:space:]]+raw\(' || true; } \
    | { grep -oE "$CALL" || true; }
}

count() { hits | grep -c . || true; }

# `file:line` of every bare string literal element in row!/column!/stack!.
macro_literals() {
  list_files | { grep -vE '/(tests|[a-z_]*_tests)\.rs$' || true; } \
    | xargs perl "$(cd "$(dirname "$0")" && pwd)/raw_label_macro_literals.pl" | tr -d '\r'
}

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# Raw-label baseline (scripts/check_raw_labels.sh). The count may only go down."
    echo "# Regenerate with: bash scripts/check_raw_labels.sh --generate"
    echo "$KEY=$(count)"
  } > "$BASELINE.tmp"
  mv "$BASELINE.tmp" "$BASELINE"
  echo "Wrote ${BASELINE#"$ROOT"/}"
  exit 0
fi

if [[ "${1:-}" != "--generate" ]]; then
  bare="$(macro_literals)"
  if [[ -n "$bare" ]]; then
    echo "Raw-label check: a bare string literal is an element of row!/column!/stack!:"
    echo "$bare" | sed 's/^/  /'
    echo "  Write text(raw(\"…\")) or text(t!(i18n, ids::…)); a &str is a widget in this iced."
    exit 1
  fi
fi

if [[ ! -f "$BASELINE" ]]; then
  echo "Raw-label ratchet: ${BASELINE#"$ROOT"/} is missing; run with --generate."
  exit 1
fi

base="$({ grep -E "^$KEY=" "$BASELINE" || true; } | tail -n 1 | cut -d= -f2 | tr -d '\r ')"
if ! [[ "$base" =~ ^[0-9]+$ ]]; then
  echo "Raw-label ratchet: no numeric baseline for '$KEY' in ${BASELINE#"$ROOT"/}."
  exit 1
fi

now="$(count)"
if (( now > base )); then
  echo "Raw-label ratchet: raw(…) call sites grew from $base to $now."
  echo "  Use t!(i18n, ids::…) for UI text or UserText for user content; raw is for"
  echo "  text the area migrations (#2057-#2062) have not reached yet."
  exit 1
fi
if (( now < base )); then
  echo "Raw-label ratchet: count $now is below the baseline $base: run"
  echo "  scripts/check_raw_labels.sh --generate and commit the baseline."
  exit 1
fi
echo "Raw-label ratchet: OK — $now raw(…) call sites (baseline $base)."
