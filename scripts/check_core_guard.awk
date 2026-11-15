# Scanner for scripts/check_core_guard.sh; read that script's header first.
#
# Reads one .rs file under app/core/. Variables: f (path to print),
# depth (module depth below app/core/), mode ("lines" or "super").
#
# mode=lines prints every code line as `f:line:text`, except that each `use`
# statement is joined over its lines and flattened into one record per path
# (`use std::{fs, io};` -> `use std::fs;` and `use std::io;`), so a group split
# over lines is judged like the one-line form.
#
# mode=super prints the records whose `super::` chain climbs above app/core/:
# more `super`s than the file's depth plus the inline `mod x {` blocks open at
# that line. Braces are counted without parsing strings, so a lone brace in a
# string can shift that count.

BEGIN { USE = "^[[:space:]]*(pub(\\([^)]*\\))?[[:space:]]+)?use[[:space:]]" }

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
  for (i = 1; i <= n; i++) {
    s = parts[i]
    if (s ~ /^[[:space:]]*$/) continue
    if (s !~ USE) { emit(start, s, start_allowed); continue }
    sub(USE, "", s)
    gsub(/[[:space:]]+/, " ", s)
    gsub(/ *\{ */, "{", s); gsub(/ *\} */, "}", s)
    gsub(/ *, */, ",", s); gsub(/ *:: */, "::", s)
    nout = 0
    flat(s, "")
    for (j = 1; j <= nout; j++) emit(start, "use " out[j] ";", start_allowed)
  }
  acc = ""
}

# Track inline `mod x {` blocks by brace depth.
function count(code, is_mod_open,   opens, closes) {
  if (is_mod_open) stack[++nmods] = braces
  opens = gsub(/\{/, "{", code); closes = gsub(/\}/, "}", code)
  braces += opens - closes
  while (nmods > 0 && braces <= stack[nmods]) nmods--
}

{ gsub(/\r/, "") }
/^[[:space:]]*\/\// { next }
{
  code = $0; sub(/\/\/.*/, "", code)
  if (acc != "") {
    acc = acc " " code
    if (code ~ /;/) flush()
    count(code, 0)
    next
  }
  if (code ~ USE) {
    acc = code; start = NR; start_allowed = depth + nmods
    if (code ~ /;/) flush()
    count(code, 0)
    next
  }
  emit(NR, $0, depth + nmods)
  count(code, code ~ /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*\{/)
}
END { if (acc != "") flush() }
