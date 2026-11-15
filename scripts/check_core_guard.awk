# Scanner for scripts/check_core_guard.sh; read that script's header first.
# POSIX awk only (runs under BSD awk, mawk and gawk).
#
# Reads one .rs file under app/core/. Variables: f (path to print),
# depth (module depth below app/core/), mode ("lines" or "super").
#
# mode=lines prints every code line as `f:line:code`, where `code` is the line
# with its `//` comment cut off (outside string literals). Each `use`
# statement, also one after `#[attr]` or `{` on the same line, is joined over
# its lines and flattened into one record per path (`use std::{fs, io};` ->
# `use std::fs;` and `use std::io;`), so a group split over lines is judged
# like the one-line form.
#
# mode=super prints the records whose `super::` chain climbs above app/core/:
# more `super`s than the file's depth plus the inline `mod x {` blocks open at
# that line. Braces are counted without parsing strings, so a lone brace in a
# string can shift that count.

BEGIN {
  PUB = "(pub(\\([^)]*\\))?[[:space:]]+)?"
  USE = "^[[:space:]]*" PUB "use[[:space:]]"
  # A `use` that starts a statement: line start, or after `]`, `{`, `}` or `;`.
  USE_ANY = "(^|[];{}])[[:space:]]*" PUB "use[[:space:]]"
  MOD_OPEN = PUB "mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\\{"
}

# The line without its `//` comment. Quote-aware for "..." strings; a char
# literal such as '"' or a raw string holding a quote can confuse it.
function strip(s,   i, c, q, out) {
  q = 0; out = ""
  for (i = 1; i <= length(s); i++) {
    c = substr(s, i, 1)
    if (q) {
      out = out c
      if (c == "\\") { i++; out = out substr(s, i, 1) } else if (c == "\"") q = 0
      continue
    }
    if (c == "\"") q = 1
    else if (c == "/" && substr(s, i + 1, 1) == "/") break
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

{ gsub(/\r/, "") }
/^[[:space:]]*\/\// { next }
{
  code = strip($0)
  if (acc != "") {
    acc = acc " " code
    if (code ~ /;/) flush()
    count(code)
    next
  }
  if (match(code, USE_ANY)) {
    # Split before the `use` keyword; the prefix is ordinary code.
    usepos = RSTART + RLENGTH - 4
    prefix = substr(code, 1, usepos - 1)
    emit(NR, prefix, depth + nmods)
    acc = substr(code, usepos); start = NR
    start_allowed = depth + nmods + (prefix ~ MOD_OPEN ? 1 : 0)
    if (acc ~ /;/) flush()
    count(code)
    next
  }
  emit(NR, code, depth + nmods)
  count(code)
}
END { if (acc != "") flush() }
