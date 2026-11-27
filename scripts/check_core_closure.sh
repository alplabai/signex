#!/usr/bin/env bash
#
# Core closure guard (#2326, ADR-0006 H7): every named type in the application
# core is reachable from the core roots, so GUI/adapter state cannot live in
# app/core/ as plain data. Decision: https://github.com/alplabai/signex/issues/2326#issuecomment-7219395928
#
#   nodes  Every struct, enum, type alias, trait and union defined at item level
#          in crates/signex-app/src/app/core.rs and app/core/**/*.rs. Skipped:
#          items directly under `#[cfg(test)]` and everything inside them
#          (scripts/uifree-test-item.yml), files whose every top-level `mod x;`
#          is under `#[cfg(test)]` (and the modules below them), and types
#          declared inside a fn, an impl or a trait. A file that is not mounted,
#          or mounted once without `#[cfg(test)]`, is production whatever its name.
#   roots  The text of: `struct DocumentState` in
#          crates/signex-app/src/app/state/mod.rs; the definitions of the
#          app/state/ types its fields name (one level: `AgentEdits`); the
#          signatures of every non-test fn in an `impl DocumentState` in
#          app/state/; and the port types in scripts/core-closure-roots.txt
#          (`Name  # why`; the comment is required). An app/state/ file adds
#          root text only when a top-level `mod x;` outside `#[cfg(test)]`
#          mounts it, up to state/mod.rs.
#   edges  A reaches B when B's name appears in A's definition (field types,
#          variant payloads, alias target, generic arguments; enum variant NAMES
#          are not edges), or in the signature (up to the body's `{`) of a
#          non-test fn in an `impl` of A inside the core.
#
# Also fails on: two core types with one name; a name in DocumentState that two
# app/state/ types share (each would add its definition); a core type named
# like a common std or dependency type (String, Vec, Uuid, ...), which would
# collect edges from that outside use; a `macro_rules!` or an item-level macro
# call in core production code, whose items the scan cannot see. `#[path]` and
# `include!` are banned in app/core/ by scripts/check_core_guard.sh.
#
# A node no root reaches is a violation. scripts/core-closure-baseline.txt lists
# the ones known today (`path Name`); it may only shrink: a new violation fails,
# so does a baseline line that is no longer one, and with `--no-growth <ref>` so
# does a baseline line that `<ref>`'s baseline does not have (a `<ref>` without
# the baseline passes only if it has no guard script either).
#
# Method: ast-grep finds the items, impl fns, macros and test ranges; the lexer of
# scripts/check_core_guard.awk drops comments and strings, so a doc comment
# naming a type is no edge. Names are matched by identifier, not resolved.
# Known blind spots: a core type name that is also a crate-external name not on
# the shadow list (`Point`) gets an edge from that outside use; a type reached
# only through a free fn or a non-core, non-app/state wrapper of DocumentState
# needs a root; a `#[path]` mount in app/state/ is not followed; a field or signature compiled out by a non-test cfg
# (`#[cfg(any())]`) still makes an edge; and a view type added as a field or in
# a signature of a reached core type is reached by design: review is the
# mitigation for that edge.
#
# Usage:
#   scripts/check_core_closure.sh                    # check
#   scripts/check_core_closure.sh --no-growth <ref>  # check, and no baseline line new against <ref> (CI)
#   scripts/check_core_closure.sh --generate         # rewrite the baseline
#
# CORE_CLOSURE_ROOT=<dir> checks another tree and CORE_CLOSURE_AWK=<awk> picks
# the awk (both used by the controls script).
#
# The awk programs and ast-grep rules are single-quoted on purpose.
# shellcheck disable=SC2016
set -euo pipefail

SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
ROOT="${CORE_CLOSURE_ROOT:-$(cd "$SCRIPTS/.." && pwd)}"
AWK="${CORE_CLOSURE_AWK:-awk}"
LEXER="$SCRIPTS/check_core_guard.awk"
TEST_RULE="$SCRIPTS/uifree-test-item.yml"
APP_SRC="crates/signex-app/src"
CORE_REL="$APP_SRC/app/core"
STATE_DIR="$APP_SRC/app/state"
STATE="$STATE_DIR/mod.rs"
ROOTS="scripts/core-closure-roots.txt"
BASELINE="scripts/core-closure-baseline.txt"

generate=0 growth_ref=""
while (( $# )); do
  case "$1" in
    --generate) generate=1 ;;
    --no-growth)
      if [[ -z "${2:-}" ]]; then
        echo "Core closure: FAIL — --no-growth needs a git ref."
        exit 2
      fi
      growth_ref="$2"
      shift
      ;;
    *) echo "Core closure: FAIL — unknown argument $1."; exit 2 ;;
  esac
  shift
done

TYPE_RULE='language: rust
rule:
  any: [{kind: struct_item}, {kind: enum_item}, {kind: type_item}, {kind: trait_item}, {kind: union_item}]
  has: {field: name, pattern: $NAME}
  not: {inside: {stopBy: end, any: [{kind: impl_item}, {kind: trait_item}, {kind: function_item}]}}'

CORE_RULES="id: core-type
$TYPE_RULE"'
---
id: core-impl-fn
language: rust
rule:
  kind: function_item
  has: {field: name, pattern: $FN}
  inside:
    kind: declaration_list
    inside: {kind: impl_item, has: {field: type, pattern: $SELF}}
---
id: core-macro
language: rust
rule:
  any:
    - kind: macro_definition
    - kind: macro_invocation
      inside:
        any:
          - kind: source_file
          - kind: declaration_list
          - kind: expression_statement
            inside: {any: [{kind: source_file}, {kind: declaration_list}]}'

STATE_RULES="id: state-type
$TYPE_RULE"

DOC_FN_RULE='id: doc-fn
language: rust
rule:
  kind: function_item
  has: {field: name, pattern: $FN}
  inside:
    kind: declaration_list
    inside: {kind: impl_item, has: {field: type, regex: "^(.*::)?DocumentState$"}}'

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null; then
    echo "Core closure: FAIL — $tool is not installed."
    exit 2
  fi
done
for f in "$ROOTS" "$STATE"; do
  if [[ ! -f "$f" ]]; then
    echo "Core closure: FAIL — no $f under $ROOT."
    exit 2
  fi
done

status=0

# Roots-file entries: `Name  # why`. A name without its reason fails.
roots="$(tr -d '\r' < "$ROOTS" | "$AWK" -v file="$ROOTS" '
  /^[[:space:]]*(#|$)/ { next }
  $0 ~ /^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]+#[[:space:]]*[^[:space:]]/ { print $1; next }
  { print "BAD\t" file ":" NR ": " $0 }')"
if grep -q '^BAD' <<<"$roots"; then
  grep '^BAD' <<<"$roots" | cut -f2- | while IFS= read -r l; do
    echo "Core closure: FAIL — $l"
  done
  echo "  Each root is one type name followed by a '# why' comment."
  status=1
fi

rs_files() { [[ -d "$1" ]] && find "$1" -type f -name '*.rs' | sort; return 0; }
core=()
[[ -f "$CORE_REL.rs" ]] && core+=("$CORE_REL.rs")
while IFS= read -r f; do core+=("$f"); done < <(rs_files "$CORE_REL")
if (( ${#core[@]} == 0 )); then
  echo "Core closure: OK — $CORE_REL does not exist yet."
  exit "$status"
fi
state_files=()
while IFS= read -r f; do state_files+=("$f"); done < <(rs_files "$STATE_DIR")

# ast_grep <flag> <rules> <files...>: `rule \t file \t start \t end \t name \t fn`.
ast_grep() {
  local flag="$1" rule="$2" scan
  shift 2
  if ! scan="$(ast-grep scan "$flag" "$rule" --json=stream "$@")"; then
    echo "Core closure: FAIL — ast-grep could not scan $*." >&2
    return 2
  fi
  jq -r '[.ruleId, .file, .range.start.line + 1, .range.end.line + 1,
          (.metaVariables.single.NAME.text // .metaVariables.single.SELF.text // ""),
          (.metaVariables.single.FN.text // "")] | @tsv' <<<"$scan"
}

# The resolver's input: items, impl fns, macros, test ranges, lexed code, roots.
feed() {
  local f
  ast_grep --inline-rules "$CORE_RULES" "${core[@]}" || return 2
  ast_grep --inline-rules "$STATE_RULES" "$STATE_DIR" || return 2
  ast_grep --inline-rules "$DOC_FN_RULE" "$STATE_DIR" || return 2
  ast_grep --rule "$TEST_RULE" "${core[@]}" "${state_files[@]}" || return 2
  for f in "${core[@]}" "${state_files[@]}"; do
    printf 'F\t%s\n' "$f"
    "$AWK" -v f="$f" -v depth=0 -v mode=lines -f "$LEXER" "$f" | "$AWK" '{ print "L\t" $0 }' || return 2
  done
  { grep -v '^BAD' <<<"$roots" || true; } | "$AWK" 'NF { print "R\t" $0 }'
}

input="$(feed)" || exit 2
if ! "$AWK" -F'\t' -v s="$STATE" '$1 == "state-type" && $2 == s && $5 == "DocumentState" { f = 1 } END { exit !f }' <<<"$input"; then
  echo "Core closure: FAIL — no struct DocumentState in $STATE."
  exit 2
fi

# V path name: unreachable; U name: a root that is no core type; D name paths:
# a shared name; E name paths: a DocumentState type name two app/state types
# share; S path name: a shadowed std name; M path:line: a macro.
result="$("$AWK" -F'\t' -v state="$STATE" -v state_dir="$STATE_DIR" -v core_rel="$CORE_REL" '
  BEGIN {
    n = split("String Vec Option Result Box Path PathBuf HashMap HashSet BTreeMap BTreeSet " \
      "VecDeque Arc Rc Cell RefCell Mutex RwLock Cow Uuid Value Map Duration Instant " \
      "SystemTime DateTime Local Utc Error Some None Ok Err Self", w, " ")
    for (i = 1; i <= n; i++) shadow[w[i]] = 1
  }
  function dirname(p) { sub(/\/[^\/]*$/, "", p); return p }
  function basename(p) { sub(/^.*\//, "", p); return p }
  function inrange(file, line,   i) {
    for (i = 1; i <= nt; i++) if (tf[i] == file && line >= ts[i] && line <= te[i]) return 1
    return 0
  }
  # Count the `mod stem;` lines at the top level of parent (an inline `mod x { }`
  # mounts another file) into dn, those under #[cfg(test)] into dt.
  function decls(parent, stem,   l, d, c, pre) {
    d = 0
    for (l = 1; l <= maxl[parent]; l++) {
      if (!((parent, l) in code)) continue
      c = code[parent, l]
      if (match(c, "(^|[^A-Za-z0-9_])mod[[:space:]]+" stem "[[:space:]]*;")) {
        pre = substr(c, 1, RSTART)
        if (d + gsub(/\{/, "{", pre) - gsub(/\}/, "}", pre) == 0) { dn++; dt += inrange(parent, l) }
      }
      d += gsub(/\{/, "{", c) - gsub(/\}/, "}", c)
    }
  }
  # Split f into the module stem and the directory its parent file names.
  function mount(f,   g) {
    g = f
    if (basename(g) == "mod.rs") g = dirname(g) ".rs"
    sub(/\.rs$/, "", g)
    mstem = basename(g); mdir = dirname(g)
    dn = 0; dt = 0
    decls(mdir ".rs", mstem); decls(mdir "/mod.rs", mstem)
  }
  function parent_of(dir) { return ((dir ".rs") in isfile) ? dir ".rs" : dir "/mod.rs" }
  # A core file is test code when every top-level `mod x;` for it is under
  # #[cfg(test)], or its parent module is test code. Unmounted: production.
  function testfile(f,   dir) {
    if (f in tfile) return tfile[f]
    tfile[f] = 0
    if (f == core_rel ".rs" || f == core_rel "/mod.rs") return 0
    mount(f)
    if (dn > 0 && dn == dt) return tfile[f] = 1
    dir = mdir
    if (dir == core_rel) return 0
    return tfile[f] = testfile(parent_of(dir))
  }
  # An app/state file adds root text only when a top-level `mod x;` outside
  # #[cfg(test)] mounts it, up to state/mod.rs. Unmounted: no root text.
  function rootsrc(f,   dir) {
    if (f in rsrc) return rsrc[f]
    rsrc[f] = 0
    if (f == state) return rsrc[f] = 1
    mount(f)
    if (dn == dt) return 0
    dir = mdir
    if (dir == state_dir) return rsrc[f] = 1
    return rsrc[f] = rootsrc(parent_of(dir))
  }
  function intest(file, line) { return testfile(file) || inrange(file, line) }
  function notroot(file, line) { return !rootsrc(file) || inrange(file, line) }
  function text(file, s, e,   l, t) {
    t = ""
    for (l = s; l <= e; l++) if ((file, l) in code) t = t " " code[file, l]
    return t
  }
  # The enum body with each variant NAME blanked: an identifier at brace depth
  # 1 right after `{` or `,` (attributes sit deeper, inside their brackets).
  function drop_variants(t,   i, c, d, out, expect, n) {
    i = index(t, "{")
    if (!i) return t
    out = substr(t, 1, i); d = 1; expect = 1; n = length(t)
    for (i++; i <= n; i++) {
      c = substr(t, i, 1)
      if (c ~ /[{([]/) { d++; out = out c; continue }
      if (c ~ /[])}]/) { d--; out = out c; continue }
      if (d == 1 && c == ",") { expect = 1; out = out c; continue }
      if (d == 1 && expect && c ~ /[A-Za-z_]/) {
        match(substr(t, i), /^[A-Za-z0-9_]+/)
        i += RLENGTH - 1; expect = 0; out = out " "; continue
      }
      out = out c
    }
    return out
  }
  # An item definition from its keyword on (a line may hold more than the item).
  function item_text(file, s, e, name,   t) {
    t = text(file, s, e)
    if (match(t, "(^|[^A-Za-z0-9_])(struct|enum|type|trait|union)[[:space:]]+" name "([^A-Za-z0-9_]|$)"))
      t = substr(t, RSTART)
    if (t ~ /^[^A-Za-z0-9_]?enum[[:space:]]/) t = drop_variants(t)
    return t
  }
  # A fn signature: from `fn name` (an impl header may share its line) to the body.
  function sig_text(file, s, e, fn,   t) {
    t = text(file, s, e)
    if (match(t, "(^|[^A-Za-z0-9_])fn[[:space:]]+" fn "([^A-Za-z0-9_]|$)")) t = substr(t, RSTART)
    sub(/[{;].*$/, "", t)
    return t
  }
  # Edges from node `from` (or the roots when empty) to every node whose name
  # is an identifier in t.
  function reach_names(t, from,   tok, n, ids, i) {
    while (match(t, /[A-Za-z_][A-Za-z0-9_]*/)) {
      tok = substr(t, RSTART, RLENGTH)
      t = substr(t, RSTART + RLENGTH)
      if (!(tok in byname)) continue
      n = split(byname[tok], ids, SUBSEP)
      for (i = 2; i <= n; i++) {
        if (from == "") root[ids[i]] = 1
        else adj[from] = adj[from] SUBSEP ids[i]
      }
    }
  }
  $1 == "L" {
    rest = substr($0, 3); p = index(rest, ":"); file = substr(rest, 1, p - 1)
    rest = substr(rest, p + 1); p = index(rest, ":"); l = substr(rest, 1, p - 1) + 0
    code[file, l] = substr(rest, p + 1)
    if (l > maxl[file]) maxl[file] = l
    next
  }
  $1 == "F" { isfile[$2] = 1; next }
  $1 == "uifree-test-item" { nt++; tf[nt] = $2; ts[nt] = $3; te[nt] = $4; next }
  $1 == "core-type" { nn++; nf[nn] = $2; ns[nn] = $3; ne[nn] = $4; nname[nn] = $5; next }
  $1 == "core-impl-fn" { nm++; mf[nm] = $2; ms[nm] = $3; me[nm] = $4; mself[nm] = $5; mfn[nm] = $6; next }
  $1 == "core-macro" { nx++; xf[nx] = $2; xs[nx] = $3; next }
  $1 == "state-type" { ns2++; sf[ns2] = $2; ss[ns2] = $3; se[ns2] = $4; sname[ns2] = $5; next }
  $1 == "doc-fn" { nd++; df[nd] = $2; dss[nd] = $3; de[nd] = $4; dfn[nd] = $6; next }
  $1 == "R" { nr++; rname[nr] = $2; next }
  END {
    for (i = 1; i <= nx; i++) if (!intest(xf[i], xs[i])) print "M\t" xf[i] ":" xs[i]
    for (i = 1; i <= nn; i++) {
      if (intest(nf[i], ns[i])) continue
      id = nf[i] " " nname[i]
      if (!(id in node)) count++
      node[id] = i
      if (nname[i] in byname) dup[nname[i]] = 1
      byname[nname[i]] = byname[nname[i]] SUBSEP id
      if (nname[i] in shadow) print "S\t" id
    }
    for (name in dup) { s = byname[name]; gsub(SUBSEP, ", ", s); print "D\t" name "\t" substr(s, 3) }
    for (id in node) { i = node[id]; reach_names(item_text(nf[i], ns[i], ne[i], nname[i]), id) }
    for (i = 1; i <= nm; i++) {
      if (intest(mf[i], ms[i])) continue
      self = mself[i]
      sub(/<.*$/, "", self); sub(/^.*[[:space:]&:]/, "", self)
      if (!(self in byname)) continue
      sig = sig_text(mf[i], ms[i], me[i], mfn[i])
      n = split(byname[self], ids, SUBSEP)
      for (k = 2; k <= n; k++) reach_names(sig, ids[k])
    }
    # Root text: DocumentState, the app/state types its fields name, the
    # signatures of its methods, and the roots file.
    for (i = 1; i <= ns2; i++) {
      if (notroot(sf[i], ss[i])) continue
      if (sf[i] == state && sname[i] == "DocumentState") docs = item_text(sf[i], ss[i], se[i], "DocumentState")
      else stype[sname[i]] = stype[sname[i]] SUBSEP i
    }
    rt = docs
    t = docs
    while (match(t, /[A-Za-z_][A-Za-z0-9_]*/)) {
      tok = substr(t, RSTART, RLENGTH); t = substr(t, RSTART + RLENGTH)
      if (!(tok in stype) || (tok in seen)) continue
      seen[tok] = 1
      n = split(stype[tok], ids, SUBSEP)
      if (n > 2) {
        s = ""
        for (k = 2; k <= n; k++) s = s ", " sf[ids[k]]
        print "E\t" tok "\t" substr(s, 3)
      }
      for (k = 2; k <= n; k++) rt = rt " " item_text(sf[ids[k]], ss[ids[k]], se[ids[k]], tok)
    }
    for (i = 1; i <= nd; i++) if (!notroot(df[i], dss[i])) rt = rt " " sig_text(df[i], dss[i], de[i], dfn[i])
    reach_names(rt, "")
    for (i = 1; i <= nr; i++) {
      if (!(rname[i] in byname)) { print "U\t" rname[i]; continue }
      reach_names(rname[i], "")
    }
    q = 0
    for (id in root) queue[++q] = id
    for (h = 1; h <= q; h++) {
      n = split(adj[queue[h]], ids, SUBSEP)
      for (k = 2; k <= n; k++) if (!(ids[k] in root)) { root[ids[k]] = 1; queue[++q] = ids[k] }
    }
    for (id in node) if (!(id in root)) print "V\t" id
    print "C\t" count + 0
  }' <<<"$input")"

# lines <tag>: the result records of one kind, tag dropped.
lines() { { grep "^$1"$'\t' <<<"$result" || true; } | cut -f2- | LC_ALL=C sort; }

while IFS= read -r name; do
  [[ -z "$name" ]] && continue
  echo "Core closure: FAIL — $ROOTS names $name, which is no type in app/core/."
  status=1
done < <(lines U)
while IFS=$'\t' read -r name where; do
  [[ -z "$name" ]] && continue
  echo "Core closure: FAIL — two app/core/ types are named $name: $where."
  echo "  Names are matched by identifier: give each core type its own name."
  status=1
done < <(lines D)
while IFS=$'\t' read -r name where; do
  [[ -z "$name" ]] && continue
  echo "Core closure: FAIL — DocumentState names $name, which two app/state/ types are named: $where."
  echo "  Its definition is root text: give each app/state/ type its own name."
  status=1
done < <(lines E)
while IFS= read -r id; do
  [[ -z "$id" ]] && continue
  echo "Core closure: FAIL — $id is named like a std or dependency type; rename it."
  status=1
done < <(lines S)
while IFS= read -r at; do
  [[ -z "$at" ]] && continue
  echo "Core closure: FAIL — $at defines or calls a macro at item level; the closure scan"
  echo "  cannot see the items it makes. Write the items out in app/core/."
  status=1
done < <(lines M)

viols="$(lines V)"
nodes="$(lines C)"

if (( generate )); then
  if (( status )); then exit 1; fi
  {
    echo "# Core closure baseline (scripts/check_core_closure.sh, #2326). Unreachable core types"
    echo "# known today, \`path Name\`. It may only shrink; delete a line when its type moves out."
    echo "# Regenerate with: bash scripts/check_core_closure.sh --generate"
    [[ -n "$viols" ]] && printf '%s\n' "$viols"
  } > "$BASELINE.tmp"
  mv "$BASELINE.tmp" "$BASELINE"
  echo "Wrote $BASELINE ($(grep -c . <<<"$viols" || true) unreachable types)."
  exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
  echo "Core closure: FAIL — $BASELINE is missing; run with --generate."
  exit 1
fi
entries() { { grep -vE '^[[:space:]]*(#|$)' || true; } | tr -d '\r' | LC_ALL=C sort; }
known="$(entries < "$BASELINE")"
new="$(LC_ALL=C comm -23 <(grep -v '^$' <<<"$viols" || true) <(grep -v '^$' <<<"$known" || true))"
stale="$(LC_ALL=C comm -13 <(grep -v '^$' <<<"$viols" || true) <(grep -v '^$' <<<"$known" || true))"

if [[ -n "$new" ]]; then
  echo "Core closure: FAIL — core types no root reaches (ADR-0006 H7):"
  awk '{ print "    " $0 }' <<<"$new"
  echo "  This is adapter state: move the type out of app/core/ to its adapter module."
  echo "  Do not add it to $ROOTS to pass; a root is a port type the host uses."
  status=1
fi
if [[ -n "$stale" ]]; then
  echo "Core closure: FAIL — $BASELINE lists types that are no longer unreachable:"
  awk '{ print "    " $0 }' <<<"$stale"
  echo "  The baseline only shrinks: delete these lines from $BASELINE."
  status=1
fi

if [[ -n "$growth_ref" ]]; then
  if ! git rev-parse -q --verify "$growth_ref^{commit}" >/dev/null; then
    echo "Core closure: FAIL — --no-growth: $growth_ref is no commit here."
    status=1
  elif ! old="$(git show "$growth_ref:$BASELINE" 2>/dev/null)"; then
    # Only the change that adds the guard has a base without either file.
    if git cat-file -e "$growth_ref:scripts/check_core_closure.sh" 2>/dev/null; then
      echo "Core closure: FAIL — --no-growth: $growth_ref has the guard but no $BASELINE."
      status=1
    else
      echo "Core closure: $growth_ref has no closure guard yet; nothing to compare."
    fi
  else
    grown="$(LC_ALL=C comm -23 <(grep -v '^$' <<<"$known" || true) <(entries <<<"$old" | grep -v '^$' || true))"
    if [[ -n "$grown" ]]; then
      echo "Core closure: FAIL — $BASELINE grew against $growth_ref:"
      awk '{ print "    " $0 }' <<<"$grown"
      echo "  The baseline only shrinks: move the type out of app/core/ instead of listing it."
      status=1
    fi
  fi
fi

if (( status )); then
  exit 1
fi
echo "Core closure: OK — $nodes core types, $(grep -c . <<<"$known" || true) unreachable on the baseline."
