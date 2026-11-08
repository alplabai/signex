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
# A second check (macro_literals below, in awk) fails on a bare string
# literal that is an element of `row![…]`, `column![…]` or `stack![…]`: `&str`
# is a widget in this iced fork, so such a literal skips `text(raw(…))`.
# ast-grep rules cover the rest (no-raw-literal-widget, no-raw-alias).
#
# Known limits: the count is per call site, so a helper that wraps `raw` once
# and is called many times counts once (crates/chrome-catalog/src/bom_modal.rs
# does this); an alias or `raw` passed as a value is refused by the ast-grep
# rule no-raw-alias, except `f(raw)` on a plain function; a byte
# string `b"…"` as a row!/column! element escapes the macro check.
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

# POSIX awk (BSD and GNU): prints `file:line` for each bare string literal that
# is an element of row![…], column![…] or stack![…]. It walks the brackets after
# the macro name, splits the top-level elements on commas, and flags an element
# that is nothing but one string literal ("…", r"…" or r#"…"#). Comments (`//`,
# `/* */`) are dropped only outside string literals; char literals are skipped;
# an inline `#[cfg(test)] mod name {` ends a file (a `mod name;` declaration
# does not); nested macros are checked too.
MACRO_AWK='
function isid(ch) { return ch ~ /[A-Za-z0-9_]/ }
function rawstart(str, i) {
  return substr(str, i, 1) == "r" && (i == 1 || !isid(substr(str, i - 1, 1))) \
    && match(substr(str, i, 40), /^r#*"/)
}
# Index of the last character of the string literal starting at i, else 0.
function litend(str, i,   j, d, h, k, term, rest, idx, len) {
  if (substr(str, i, 1) == "\"") {
    j = i + 1
    while (j <= length(str)) {
      d = substr(str, j, 1)
      if (d == "\\") { j += 2; continue }
      if (d == "\"") return j
      j++
    }
    return 0
  }
  if (!rawstart(str, i)) return 0
  len = RLENGTH; h = len - 2; term = "\""
  for (k = 0; k < h; k++) term = term "#"
  rest = substr(str, i + len)
  idx = index(rest, term)
  if (!idx) return 0
  return i + len + idx - 2 + length(term)
}
function strip(b,   out, i, n, c, e) {
  out = ""; i = 1; n = length(b)
  while (i <= n) {
    c = substr(b, i, 1)
    e = (c == "\"" || c == "r") ? litend(b, i) : 0
    if (e) { out = out substr(b, i, e - i + 1); i = e + 1; continue }
    if (c == "\047" && match(substr(b, i, 12), /^\047(\\[^\047]*|[^\\\047])\047/)) {
      out = out substr(b, i, RLENGTH); i += RLENGTH; continue
    }
    if (c == "/" && substr(b, i + 1, 1) == "/") {
      while (i <= n && substr(b, i, 1) != "\n") i++
      continue
    }
    if (c == "/" && substr(b, i + 1, 1) == "*") {
      e = index(substr(b, i + 2), "*/")
      e = e ? i + 2 + e : n
      while (i <= e) { c = substr(b, i, 1); if (c == "\n") out = out c; i++ }
      continue
    }
    out = out c; i++
  }
  return out
}
function check(from, to,   body, lead, nl, e, t) {
  body = strip(substr(s, from, to - from))
  match(body, /^[ \t\n]*/)
  lead = substr(body, 1, RLENGTH)
  e = litend(body, RLENGTH + 1)
  if (!e || substr(body, e + 1) !~ /^[ \t\n]*$/) return
  nl = gsub(/\n/, "\n", lead)
  t = substr(s, 1, from - 1)
  print FILENAME ":" (gsub(/\n/, "\n", t) + nl + 1)
}
{
  if (stop) next
  if (pend && $0 ~ /^[ \t]*(pub(\([a-z]+\))?[ \t]+)?mod[ \t]+[A-Za-z0-9_]+[ \t]*\{/) { stop = 1; next }
  pend = ($0 ~ /^[ \t]*#\[cfg\(test\)\]/)
  s = s $0 "\n"
}
END {
  n = length(s); p = 1
  while (p <= n) {
    if (!match(substr(s, p), /(row|column|stack)!\[/)) break
    st = p + RSTART - 1; i = st + RLENGTH; p = i
    if (st > 1 && isid(substr(s, st - 1, 1))) continue
    depth = 0; from = i
    while (i <= n) {
      c = substr(s, i, 1)
      e = (c == "\"" || c == "r") ? litend(s, i) : 0
      if (e) {
        i = e + 1
        continue
      } else if (c == "\047") {
        if (match(substr(s, i, 12), /^\047(\\[^\047]*|[^\\\047])\047/)) i += RLENGTH - 1
      } else if (c == "/" && substr(s, i + 1, 1) == "/") {
        while (i <= n && substr(s, i, 1) != "\n") i++
        continue
      } else if (c == "/" && substr(s, i + 1, 1) == "*") {
        e = index(substr(s, i + 2), "*/")
        i = e ? i + 2 + e + 1 : n + 1
        continue
      } else if (c ~ /[[({]/) {
        depth++
      } else if (c ~ /[)}]/) {
        depth--
      } else if (c == "]") {
        if (depth == 0) { check(from, i); break }
        depth--
      } else if (c == "," && depth == 0) {
        check(from, i); from = i + 1
      }
      i++
    }
  }
}'

# `file:line` of every bare string literal element in row!/column!/stack!.
macro_literals() {
  list_files | { grep -vE '/(tests|[a-z_]*_tests)\.rs$' || true; } \
    | xargs -n 1 env LC_ALL=C awk "$MACRO_AWK" | tr -d '\r'
}

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# Raw-label baseline (scripts/check_raw_labels.sh). The count must equal this number (lower it with --generate in the same change)."
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
