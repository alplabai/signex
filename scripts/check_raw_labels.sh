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
# literal that is an element of `row!`, `column!`, `stack!`, `grid!` or
# `rich_text!` (any of `[…]`, `(…)`, `{…}`): `&str` is a widget in this iced
# fork, so such a literal skips `text(raw(…))`. A literal wrapped as `("X")`,
# `{ "X" }` or in an `if … { "X" } else { … }` branch counts as bare too.
# ast-grep rules cover the rest (no-raw-literal-widget, no-raw-alias).
#
# Known limits: the count is per call site, so a helper that wraps `raw` once
# and is called many times counts once (crates/chrome-catalog/src/bom_modal.rs
# does this); an alias or `raw` passed as a value is refused by the ast-grep
# rule no-raw-alias, except `f(raw)` on a plain function. The macro check sees
# literals only: a `const LABEL: &str` or a `let` bound to a literal and then
# used as an element, and a literal reached through a `match` arm, pass.
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
# is an element of a layout macro. Each line is first masked, one character
# for one, into `m`: comments (nested `/* */` too) become spaces, a plain
# string literal ("…", r"…", r#"…"#) becomes \001 then \002s, and any other
# literal (b"…", br#"…"#, c"…", cr"…", a char) becomes \003s. The macro search
# and the bracket matching then run on `m`, so a comment or a string never
# opens a macro or moves a bracket. An inline `#[cfg(test)] mod name { … }` is
# skipped up to its closing brace (a `mod name;` declaration is not); nested
# macros are checked too.
MACRO_AWK='
function isid(ch) { return ch ~ /[A-Za-z0-9_]/ }
function rep(ch, k,   o) { o = ""; while (k-- > 0) o = o ch; return o }
# Index of the delimiter that closes the one at o, else 0.
function closer(o,   i, d, ch) {
  d = 0
  for (i = o; i <= n; i++) {
    ch = substr(m, i, 1)
    if (ch == "(" || ch == "[" || ch == "{") d++
    else if (ch == ")" || ch == "]" || ch == "}") { d--; if (!d) return i }
  }
  return 0
}
# Start of the bare literal that m[a..b] evaluates to, else 0.
function bare(a, b,   ch, t, e, i, d, r, last) {
  while (a <= b && substr(m, a, 1) ~ /[ \t\r\n]/) a++
  while (b >= a && substr(m, b, 1) ~ /[ \t\r\n]/) b--
  if (a > b) return 0
  ch = substr(m, a, 1)
  if (ch == "\001") {
    t = substr(m, a + 1, b - a); gsub(/[\002\r\n]/, "", t)
    return t == "" ? a : 0
  }
  if (ch == "(" && closer(a) == b) return bare(a + 1, b - 1)
  if (ch == "{" && closer(a) == b) {
    last = a; d = 0
    for (i = a + 1; i < b; i++) {
      ch = substr(m, i, 1)
      if (ch ~ /[[({]/) d++
      else if (ch ~ /[])}]/) d--
      else if (ch == ";" && !d) last = i
    }
    return bare(last + 1, b - 1)
  }
  if (substr(m, a, 2) == "if" && !isid(substr(m, a + 2, 1))) {
    d = 0
    for (i = a + 2; i <= b; i++) {
      ch = substr(m, i, 1)
      if (ch == "{" && !d) {
        e = closer(i); if (!e) return 0
        r = bare(i, e); if (r) return r
        i = e; continue
      }
      if (ch ~ /[[({]/) d++
      else if (ch ~ /[])}]/) d--
    }
  }
  return 0
}
function flag(a, b,   r, t) {
  r = bare(a, b)
  if (!r) return
  t = substr(m, 1, r - 1)
  print FILENAME ":" (gsub(/\n/, "\n", t) + 1)
}
function intest(pos,   k) {
  for (k = 1; k <= nt; k++) if (pos >= ta[k] && pos <= tb[k]) return 1
  return 0
}
{
  line = $0; n = length(line); out = ""; i = 1
  while (i <= n) {
    c = substr(line, i, 1)
    if (cdepth) {
      t = substr(line, i, 2)
      if (t == "/*") { cdepth++; out = out "  "; i += 2 }
      else if (t == "*/") { cdepth--; out = out "  "; i += 2 }
      else { out = out " "; i++ }
      continue
    }
    if (inlit) {
      if (hashes < 0 && c == "\\") { k = (i < n) ? 2 : 1; out = out rep(fill, k); i += k; continue }
      if (c == "\"" && (hashes <= 0 || substr(line, i + 1, hashes) == rep("#", hashes))) {
        k = 1 + (hashes > 0 ? hashes : 0); out = out rep(fill, k); i += k; inlit = 0; continue
      }
      out = out fill; i++; continue
    }
    if ((c == "\"" || ((c == "b" || c == "c" || c == "r") && (i == 1 || !isid(substr(line, i - 1, 1))))) \
        && match(substr(line, i, 300), /^([bc]?r#*|[bc])?"/)) {
      k = RLENGTH; pre = substr(line, i, k - 1)
      hashes = (pre ~ /r/) ? gsub(/#/, "#", pre) : -1
      fill = (pre ~ /[bc]/) ? "\003" : "\002"
      out = out (fill == "\002" ? "\001" : "\003") rep(fill, k - 1); i += k; inlit = 1; continue
    }
    if (c == "\047" && match(substr(line, i, 16), /^\047(\\.[^\047]*|[^\\\047])\047/)) {
      out = out rep("\003", RLENGTH); i += RLENGTH; continue
    }
    if (c == "/" && substr(line, i + 1, 1) == "/") { out = out rep(" ", n - i + 1); i = n + 1; continue }
    if (c == "/" && substr(line, i + 1, 1) == "*") { cdepth = 1; out = out "  "; i += 2; continue }
    out = out c; i++
  }
  m = m out "\n"
}
END {
  n = length(m); p = 1; nt = 0
  while (match(substr(m, p), /#\[cfg\(test\)\]/)) {
    a = p + RSTART - 1 + RLENGTH; p = a
    if (match(substr(m, a, 400), /^([ \t\r\n]*#\[[^]]*\])*[ \t\r\n]*(pub(\([a-z ]+\))?[ \t\r\n]+)?mod[ \t\r\n]+[A-Za-z0-9_]+[ \t\r\n]*\{/)) {
      o = a + RLENGTH - 1; e = closer(o)
      if (!e) e = n
      nt++; ta[nt] = o; tb[nt] = e; p = e + 1
    }
  }
  p = 1
  while (match(substr(m, p), /(row|column|stack|grid|rich_text)![ \t\r\n]*[[({]/)) {
    st = p + RSTART - 1; o = st + RLENGTH - 1; p = o + 1
    if (st > 1 && isid(substr(m, st - 1, 1))) continue
    if (intest(st)) continue
    e = closer(o)
    if (!e) continue
    from = o + 1; d = 0
    for (i = from; i < e; i++) {
      c = substr(m, i, 1)
      if (c ~ /[[({]/) d++
      else if (c ~ /[])}]/) d--
      else if (c == "," && !d) { flag(from, i - 1); from = i + 1 }
    }
    flag(from, e - 1)
  }
}'

# `file:line` of every bare string literal element in a layout macro.
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
    echo "Raw-label check: a bare string literal is an element of row!/column!/stack!/grid!/rich_text!:"
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
