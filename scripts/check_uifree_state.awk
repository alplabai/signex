# Resolver for scripts/check_uifree_state.sh; read that script's header first.
# POSIX awk only (runs under BSD awk, mawk and gawk).
#
# Judges one .rs file. Input records, all read before anything is judged:
#   T<TAB>first<TAB>last           a #[cfg(test)] item's line range (1-based)
#   L:<line>:<code>                a code line from scripts/check_core_guard.awk
#                                  (mode=lines: comments and strings gone, each
#                                  `use` flattened to one `use path;` record)
#   F<TAB>line<TAB>struct<TAB>field<TAB>type   a struct field from ast-grep
# Variables: mode ("fields" or "closure"), file (path to print), modpath (the
# file's module, `crate::app::state`), allowed (crate idents allowed as an
# origin, space separated), deps (signex-app's dependency idents), listed (the
# UI-free module paths), modules (a file of every signex-app module path),
# wanted (`Struct=glob,glob;Struct=*` for mode=fields).
#
# mode=fields prints, per counted field, `COUNT<TAB>Struct.field<TAB>report`,
# and `SEEN<TAB>Struct` per wanted struct found. mode=closure prints
# `VIOL<TAB>report` per import a listed module may not make.

BEGIN {
  FS = "\n"
  split(allowed, a, " "); for (i in a) ok_crate[a[i]] = 1
  split(deps, a, " "); for (i in a) dep[a[i]] = 1
  split(listed, a, " "); for (i in a) if (a[i] != "") is_listed[a[i]] = 1
  while ((getline m < modules) > 0) is_module[m] = 1
  close(modules)
  split("std core alloc", a, " "); for (i in a) std_root[a[i]] = 1
  split("bool char str u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64", a, " ")
  for (i in a) prim[a[i]] = 1
  # The std prelude names a field type can use unqualified.
  split("Option Some None Result Ok Err String Vec Box ToString ToOwned Clone Copy Send Sync Sized Unpin Drop Fn FnMut FnOnce Default Eq PartialEq Ord PartialOrd Iterator IntoIterator DoubleEndedIterator ExactSizeIterator Extend From Into TryFrom TryInto AsRef AsMut", a, " ")
  for (i in a) prelude[a[i]] = 1
  split("dyn impl mut const fn for unsafe extern as where static _", a, " ")
  for (i in a) keyword[a[i]] = 1
  nseg = split(modpath, modseg, "::")
  n = split(wanted, a, ";")
  for (i = 1; i <= n; i++) {
    if (a[i] == "") continue
    k = index(a[i], "=")
    globs[substr(a[i], 1, k - 1)] = substr(a[i], k + 1)
  }
  PATH_RE = "(::)?[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*"
}

/^T\t/ { split($0, p, "\t"); nt++; tlo[nt] = p[2] + 0; thi[nt] = p[3] + 0; next }
/^L:/ {
  r = substr($0, 3); k = index(r, ":")
  nl++; lno[nl] = substr(r, 1, k - 1) + 0; lcode[nl] = substr(r, k + 1)
  next
}
/^F\t/ {
  split($0, p, "\t"); nfld++
  fln[nfld] = p[2] + 0; fst[nfld] = p[3]; fnm[nfld] = p[4]; fty[nfld] = p[5]
  next
}

function in_test(ln,   i) {
  for (i = 1; i <= nt; i++) if (ln >= tlo[i] && ln <= thi[i]) return 1
  return 0
}

function head_of(p,   k) { k = index(p, "::"); return k ? substr(p, 1, k - 1) : p }
function rest_of(p,   k) { k = index(p, "::"); return k ? substr(p, k) : "" }
function last_seg(p) { sub(/^.*::/, "", p); return p }

# `super::super::x` against modpath; "" when it climbs above the crate root.
function up(p,   k, i, out) {
  k = 0
  while (p ~ /^super(::|$)/) { k++; sub(/^super(::)?/, "", p) }
  if (k >= nseg) return ""
  out = modseg[1]
  for (i = 2; i <= nseg - k; i++) out = out "::" modseg[i]
  return p == "" ? out : out "::" p
}

# A `use` path made absolute: `crate::...`, a crate ident first, or "".
function use_abs(p,   h) {
  if (p ~ /^::/) return substr(p, 3)
  h = head_of(p)
  if (h == "crate") return p
  if (h == "self") return modpath rest_of(p)
  if (h == "super") return up(p)
  if (h in std_root || h in dep) return p
  # Edition 2018: any other first segment is an item of this module.
  return modpath "::" p
}

# A path named in code or a type, made absolute. "@std" for a primitive or
# prelude name, "@self" for `Self`, "" when nothing binds it.
function resolve(p,   h, r) {
  if (p ~ /^::/) return substr(p, 3)
  h = head_of(p); r = rest_of(p)
  if (h == "crate") return p
  if (h == "self") return modpath r
  if (h == "super") return up(p)
  if (h == "Self") return "@self"
  if (h in usemap) return usemap[h] == "@ambiguous" ? "" : usemap[h] r
  if (h in local || (r == "" && h in localval)) return modpath "::" p
  if (r == "" && h in prim) return "@std"
  if (h in prelude) return "@std"
  if (h in std_root || h in dep) return p
  return ""
}

# "" when the absolute path's origin is allowed, else the origin to report.
function classify(full,   h, parent) {
  if (full == "") return "unresolved"
  if (full == "@std" || full == "@self") return ""
  h = head_of(full)
  if (h in std_root) return ""
  if (h == "crate") {
    if (full == "crate::app::core" || index(full, "crate::app::core::") == 1) return ""
    if (full in is_listed) return ""
    if (full in is_module) return full
    parent = full; sub(/::[^:]*$/, "", parent)
    if (parent == full) return full
    if (parent in is_listed) return ""
    return parent
  }
  if (h in ok_crate) return ""
  return h
}

# Bind `name` to `target` at module scope; a name bound to two different
# paths (a `use` and a `mod` of the same name, say) is ambiguous.
function bind_name(name, target) {
  if (name in usemap && usemap[name] != target) usemap[name] = "@ambiguous"
  else usemap[name] = target
}

# The `use` bindings, type-namespace items (and child modules) and
# value-namespace items (`const` / `static`) at module scope of the production
# code. A `use` or item inside a fn or inline `mod` body (brace depth > 0)
# binds nothing at module scope, so it is not collected.
function collect(   i, c, body, path, bind, k, t, name, kind, depth, opens, closes, at_top) {
  depth = 0
  for (i = 1; i <= nl; i++) {
    c = lcode[i]
    at_top = depth == 0
    t = c; opens = gsub(/\{/, "{", t); closes = gsub(/\}/, "}", t)
    depth += opens - closes
    if (depth < 0) depth = 0
    if (!at_top || in_test(lno[i])) continue
    if (c ~ /^use /) {
      body = c; sub(/^use /, "", body); sub(/;[[:space:]]*$/, "", body)
      bind = ""
      if ((k = index(body, " as ")) > 0) { bind = substr(body, k + 4); path = substr(body, 1, k - 1) }
      else path = body
      gsub(/[[:space:]]/, "", bind)
      if (path ~ /(^|::)\*$/) continue
      sub(/::self$/, "", path)
      if (bind == "") bind = last_seg(path)
      if (bind != "_") bind_name(bind, use_abs(path))
      continue
    }
    t = c
    while (match(t, /(^|[^A-Za-z0-9_])(struct|enum|union|trait|type|const|static|mod)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)) {
      name = substr(t, RSTART, RLENGTH)
      t = substr(t, RSTART + RLENGTH)
      kind = name; sub(/[[:space:]]+[A-Za-z_0-9]+$/, "", kind); sub(/^.*[^a-z]/, "", kind)
      sub(/^.*[[:space:]]/, "", name)
      # A `const` / `static` lives in the value namespace: it never shadows
      # the first segment of a path such as `iced::Color`.
      if (kind == "const" || kind == "static") { localval[name] = 1; continue }
      local[name] = 1
      if (kind == "mod") localmod[name] = 1
    }
  }
}

# Shell-style globs (`*` only) joined by commas; 1 when `name` matches a glob
# and no `!glob` exclusion.
function glob_match(name, list,   n, g, i, re, neg, hit) {
  n = split(list, g, ",")
  hit = 0
  for (i = 1; i <= n; i++) {
    re = g[i]; neg = sub(/^!/, "", re); gsub(/\*/, ".*", re)
    if (name !~ ("^" re "$")) continue
    if (neg) return 0
    hit = 1
  }
  return hit
}

# The disallowed origins a type names, comma separated, each once.
function type_origins(ty,   t, tok, after, o, out, seen) {
  t = ty; gsub(/'[A-Za-z_][A-Za-z0-9_]*/, "", t)
  out = ""
  while (match(t, PATH_RE)) {
    tok = substr(t, RSTART, RLENGTH); after = substr(t, RSTART + RLENGTH)
    t = after
    if (tok in keyword) continue
    # An associated type binding (`Item = X`) names no type.
    if (after ~ /^[[:space:]]*=[^=]/) continue
    o = classify(resolve(tok))
    if (o != "" && !((o) in seen)) { seen[o] = 1; out = out (out == "" ? "" : ", ") o }
  }
  return out
}

function fields(   i, s, name, o, idx) {
  for (i = 1; i <= nfld; i++) {
    s = fst[i]
    if (!(s in globs) || in_test(fln[i])) continue
    name = fnm[i]
    if (name == "") name = idx[s]++
    if (!glob_match(name, globs[s])) continue
    seen_struct[s] = 1
    o = type_origins(fty[i])
    if (o != "") print "COUNT\t" s "." name "\t" file ":" fln[i] " " s "." name ": " fty[i] " (" o ")"
  }
  for (s in seen_struct) print "SEEN\t" s
}

function viol(ln, what, o) { print "VIOL\t" file ":" ln " " what " (" o ")" }

function closure(   i, c, body, path, k, t, tok, pre, h, r, o) {
  for (i = 1; i <= nl; i++) {
    if (in_test(lno[i])) continue
    c = lcode[i]
    if (c ~ /^use /) {
      body = c; sub(/^use /, "", body); sub(/;[[:space:]]*$/, "", body)
      if ((k = index(body, " as ")) > 0) body = substr(body, 1, k - 1)
      sub(/(::)?\*$/, "", body); sub(/::self$/, "", body)
      path = body == "" ? "" : use_abs(body)
      o = classify(path)
      if (o != "") viol(lno[i], "imports " (path == "" ? body : path), o)
      continue
    }
    if (c ~ /#!?[[:space:]]*\[[[:space:]]*(cfg_attr\(.*,[[:space:]]*)?path[[:space:]]*=/) viol(lno[i], "mounts a file with #[path]", "path attribute")
    if (c ~ /(^|[^A-Za-z0-9_])include[[:space:]]*!/) viol(lno[i], "pastes source with include!", "include")
    if (match(c, /extern[[:space:]]+crate[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)) {
      tok = substr(c, RSTART, RLENGTH); sub(/^.*[[:space:]]/, "", tok)
      o = classify(tok == "self" ? "crate" : tok)
      if (o != "") viol(lno[i], "links extern crate " tok, o)
    }
    t = c
    while (match(t, PATH_RE)) {
      tok = substr(t, RSTART, RLENGTH)
      pre = RSTART > 1 ? substr(t, RSTART - 1, 1) : ""
      t = substr(t, RSTART + RLENGTH)
      if (index(tok, "::") == 0) continue
      # `<T as Trait>::f`: a qualified path, its parts were judged as tokens.
      if (pre == ">" && tok ~ /^::/) continue
      h = head_of(tok)
      if (h == "Self" || h in prim) continue
      # A judged `use` binds it, or it is an item of this module.
      if (h in usemap || (h in local && !(h in localmod))) continue
      r = resolve(tok)
      if (r == "") {
        # An unbound capitalised head is a type or variant a judged `use` or
        # a generic brought in; a lowercase one is a module of this file.
        if (h ~ /^[A-Z]/) continue
        r = (h == "super") ? "" : modpath "::" tok
      }
      if (r == "@std" || r == "@self") continue
      o = classify(r)
      if (o != "") viol(lno[i], "names " tok, o)
    }
  }
}

END {
  collect()
  if (mode == "fields") fields()
  else closure()
}
