# Scanner for scripts/check_core_guard.sh; read that script's header first.
# POSIX awk only (runs under BSD awk, mawk and gawk).
#
# Reads one .rs file under app/core/. Variables: f (path to print),
# depth (module depth below app/core/), mode ("lines" or "super").
#
# Every line is lexed first (strip): `//` and `/* */` comments are dropped,
# string, raw-string and char literal contents are blanked to "" and '', and
# the `r#` of a raw identifier is cut, across lines. So no comment or string
# can hide code, open a `mod x {`, shift the brace count or name `super`.
#
# mode=lines prints every code line as `f:line:code`. An attribute whose
# brackets stay open is joined over its lines first. Each `use` statement,
# also one after `#[attr]` or `{` on the same line, is joined over its lines
# and flattened into one record per path (`use std::{fs, io};` ->
# `use std::fs;` and `use std::io;`), so a group split over lines is judged
# like the one-line form.
#
# mode=super prints the records whose `super::` chain climbs above app/core/:
# more `super`s than the file's depth plus the inline `mod x {` blocks open at
# that line.

BEGIN {
  PUB = "(pub(\\([^)]*\\))?[[:space:]]+)?"
  USE = "^[[:space:]]*" PUB "use[[:space:]]"
  # A `use` that starts a statement: line start, or after `]`, `{`, `}` or `;`.
  USE_ANY = "(^|[];{}])[[:space:]]*" PUB "use[[:space:]]"
  MOD_OPEN = PUB "mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\\{"
  bc = 0; sq = 0; rh = 0
}

function ident(c) { return c ~ /^[A-Za-z0-9_]$/ }

# A raw string or raw identifier may start at s[i] == "r": not glued to a
# preceding identifier, except a `b` / `c` literal prefix.
function raw_at(s, i,   p) {
  p = i > 1 ? substr(s, i - 1, 1) : ""
  if (p == "b" || p == "c") p = i > 2 ? substr(s, i - 2, 1) : ""
  return !ident(p)
}

# The line as code, lexer state (bc: block-comment depth, sq: 1 inside a
# string, 2 inside a raw string closed by `"` and rh `#`s) carried over lines.
function strip(s,   i, j, n, c, d, out) {
  out = ""; n = length(s)
  for (i = 1; i <= n; i++) {
    c = substr(s, i, 1); d = substr(s, i, 2)
    if (bc) {
      if (d == "/*") { bc++; i++ }
      else if (d == "*/") { bc--; i++; if (!bc) out = out " " }
      continue
    }
    if (sq == 1) {
      if (c == "\\") i++
      else if (c == "\"") { sq = 0; out = out c }
      continue
    }
    if (sq == 2) {
      if (c == "\"" && substr(s, i + 1, rh) == substr("################", 1, rh)) {
        sq = 0; i += rh; out = out c
      }
      continue
    }
    if (d == "//") break
    if (d == "/*") { bc = 1; i++; continue }
    if (c == "\"") { sq = 1; out = out c; continue }
    if (c == "r" && raw_at(s, i)) {
      for (j = i + 1; substr(s, j, 1) == "#"; j++) ;
      if (substr(s, j, 1) == "\"") { sq = 2; rh = j - i - 1; i = j; out = out "\""; continue }
      if (j == i + 2 && substr(s, j, 1) ~ /[A-Za-z_]/) { i++; continue }
    }
    if (c == "'") {
      if (substr(s, i + 1, 1) == "\\") {
        for (j = i + 3; j <= n && substr(s, j, 1) != "'"; j++) ;
        i = j; out = out "''"; continue
      }
      # One ASCII char, or the 2-4 bytes of one non-ASCII char ('é' when awk
      # counts bytes); a lifetime holds ASCII before any later quote.
      for (j = i + 2; j <= i + 5 && substr(s, j, 1) != "'"; j++) ;
      if (j == i + 2 || (j <= i + 5 && substr(s, i + 1, j - i - 1) !~ /[ -~]/)) {
        i = j; out = out "''"; continue
      }
    }
    out = out c
  }
  return out
}

# Most `super`s in one chain on the line, ignoring `xsuper`-style identifiers.
function supers(t,   best, n, pre, seg) {
  best = 0
  while (match(t, /(super::)*super([^A-Za-z0-9_]|$)/)) {
    pre = RSTART > 1 ? substr(t, RSTART - 1, 1) : ""
    seg = substr(t, RSTART, RLENGTH)
    n = gsub(/super/, "super", seg)
    if (pre !~ /[A-Za-z0-9_]/ && n > best) best = n
    t = substr(t, RSTART + RLENGTH)
  }
  return best
}

function emit(line, text, allowed) {
  if (text ~ /^[[:space:]]*$/) return
  if (mode == "lines") print f ":" line ":" text
  else if (supers(text) > allowed) print f ":" line ":" text
}

# Expand a whitespace-normalised use tree into flat paths in out[].
function flat(s, pre,   open, head, d, i, c, item) {
  sub(/^ /, "", s); sub(/ $/, "", s)
  open = index(s, "{")
  if (open == 0) { if (s != "") out[++nout] = pre s; return }
  head = substr(s, 1, open - 1)
  d = 0; item = ""
  for (i = open + 1; i <= length(s); i++) {
    c = substr(s, i, 1)
    if (c == "{") { d++; item = item c; continue }
    if (c == "}") { if (d == 0) break; d--; item = item c; continue }
    if (c == "," && d == 0) { flat(item, pre head); item = ""; continue }
    item = item c
  }
  flat(item, pre head)
}

function flush(   n, parts, i, s, j) {
  n = split(acc, parts, ";")
  acc = ""
  for (i = 1; i <= n; i++) {
    s = parts[i]
    if (s !~ USE) { emit(start, s, start_allowed); continue }
    # A `use` after the last `;` runs on to the next lines: keep joining it.
    if (i == n) { acc = s; return }
    sub(USE, "", s)
    gsub(/[[:space:]]+/, " ", s)
    gsub(/ *\{ */, "{", s); gsub(/ *\} */, "}", s)
    gsub(/ *, */, ",", s); gsub(/ *:: */, "::", s)
    nout = 0
    flat(s, "")
    for (j = 1; j <= nout; j++) emit(start, "use " out[j] ";", start_allowed)
  }
}

# Track inline `mod x {` blocks by brace depth.
function count(code,   opens, closes) {
  if (code ~ MOD_OPEN) stack[++nmods] = braces
  opens = gsub(/\{/, "{", code); closes = gsub(/\}/, "}", code)
  braces += opens - closes
  while (nmods > 0 && braces <= stack[nmods]) nmods--
}

# `[` minus `]` on a stripped line.
function brackets(t,   o) {
  o = gsub(/\[/, "[", t)
  return o - gsub(/\]/, "]", t)
}

# Judge one stripped line (or joined attribute) that starts at source line ln.
function process(code, ln,   usepos, prefix) {
  if (acc != "") {
    acc = acc " " code
    if (code ~ /;/) flush()
    count(code)
    return
  }
  if (match(code, USE_ANY)) {
    # Split before the `use` keyword; the prefix is ordinary code.
    usepos = RSTART + RLENGTH - 4
    prefix = substr(code, 1, usepos - 1)
    emit(ln, prefix, depth + nmods)
    acc = substr(code, usepos); start = ln
    start_allowed = depth + nmods + (prefix ~ MOD_OPEN ? 1 : 0)
    if (acc ~ /;/) flush()
    count(code)
    return
  }
  emit(ln, code, depth + nmods)
  count(code)
}

{
  gsub(/\r/, "")
  code = strip($0)
  # An attribute left open (rustfmt splits `#[cfg_attr(..., path = ...)]`)
  # is joined until its brackets close, so it is judged as one line.
  if (attr != "") {
    attr = attr " " code
    if (brackets(attr) <= 0) { process(attr, attr_start); attr = "" }
    next
  }
  if (acc == "" && code ~ /#!?[[:space:]]*\[/ && brackets(code) > 0) {
    attr = code; attr_start = NR
    next
  }
  process(code, NR)
}
END {
  if (attr != "") process(attr, attr_start)
  if (acc != "") flush()
}
