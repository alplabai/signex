#!/usr/bin/env bash
#
# Core closure guard (#2326, ADR-0006 H7): every named type in the application
# core is reachable from the core roots, so GUI/adapter state cannot live in
# app/core/ as plain data. Decision: https://github.com/alplabai/signex/issues/2326#issuecomment-7219395928
#
#   nodes  Every struct, enum, type alias, trait and union defined at item level
#          in crates/signex-app/src/app/core.rs and app/core/**/*.rs. Skipped:
#          `tests.rs` / `*_tests.rs` files, items directly under `#[cfg(test)]`
#          and everything inside them (scripts/uifree-test-item.yml), and types
#          declared inside a fn, an impl or a trait.
#   roots  Every type named in `struct DocumentState` of
#          crates/signex-app/src/app/state/mod.rs, plus the port types in
#          scripts/core-closure-roots.txt (`Name  # why`; the comment is required).
#   edges  A reaches B when B's name appears in A's definition (fields, variant
#          payloads, alias target, generic arguments), or in the signature (up to
#          the body's `{`) of a non-test fn in an `impl` of A inside the core.
#
# A node no root reaches is a violation. scripts/core-closure-baseline.txt lists
# the ones known today (`path Name`); it may only shrink: a new violation fails,
# and so does a baseline line that is no longer one.
#
# Method: ast-grep finds the items, impl fns and test ranges; the lexer of
# scripts/check_core_guard.awk drops comments and strings, so a doc comment
# naming a type is no edge. Names are matched by identifier, not resolved.
# Known blind spots: two core types with one name are one name (a reference to
# either reaches both); a core type name that is also a std or crate-external
# name (`Result`, `Point`) gets an edge from that outside use; a type reached
# only through a free fn, a macro or a non-core wrapper type of DocumentState
# counts as unreachable and needs a root; a file mounted by `#[cfg(test)] mod x;`
# under another name than `*tests.rs` counts as production (fail-safe).
#
# Usage:
#   scripts/check_core_closure.sh              # check (CI)
#   scripts/check_core_closure.sh --generate   # rewrite the baseline
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
CORE_REL="crates/signex-app/src/app/core"
STATE="crates/signex-app/src/app/state/mod.rs"
ROOTS="scripts/core-closure-roots.txt"
BASELINE="scripts/core-closure-baseline.txt"

ITEM_RULES='id: core-type
language: rust
rule:
  any: [{kind: struct_item}, {kind: enum_item}, {kind: type_item}, {kind: trait_item}, {kind: union_item}]
  has: {field: name, pattern: $NAME}
  not: {inside: {stopBy: end, any: [{kind: impl_item}, {kind: trait_item}, {kind: function_item}]}}
---
id: core-impl-fn
language: rust
rule:
  kind: function_item
  has: {field: name, pattern: $FN}
  inside:
    kind: declaration_list
    inside: {kind: impl_item, has: {field: type, pattern: $SELF}}'

STATE_RULE='id: document-state
language: rust
rule:
  kind: struct_item
  has: {field: name, regex: "^DocumentState$"}'

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

# Every core file but the dedicated test files.
core_files() {
  [[ -f "$CORE_REL.rs" ]] && echo "$CORE_REL.rs"
  [[ -d "$CORE_REL" ]] && find "$CORE_REL" -type f -name '*.rs' | sort
  return 0
}
files=()
while IFS= read -r f; do
  case "$(basename "$f")" in tests.rs | *_tests.rs) continue ;; esac
  files+=("$f")
done < <(core_files)
if (( ${#files[@]} == 0 )); then
  echo "Core closure: OK — $CORE_REL does not exist yet."
  exit "$status"
fi

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

# The resolver's input: items, impl fns, test ranges, lexed code, roots.
feed() {
  local f
  ast_grep --inline-rules "$ITEM_RULES" "${files[@]}" || return 2
  ast_grep --rule "$TEST_RULE" "${files[@]}" || return 2
  ast_grep --inline-rules "$STATE_RULE" "$STATE" || return 2
  for f in "${files[@]}" "$STATE"; do
    "$AWK" -v f="$f" -v depth=0 -v mode=lines -f "$LEXER" "$f" | "$AWK" '{ print "L\t" $0 }' || return 2
  done
  { grep -v '^BAD' <<<"$roots" || true; } | "$AWK" 'NF { print "R\t" $0 }'
}

input="$(feed)" || exit 2
if ! grep -q '^document-state' <<<"$input"; then
  echo "Core closure: FAIL — no struct DocumentState in $STATE."
  exit 2
fi

# V path name: unreachable nodes; U name: a root that is no core type.
result="$("$AWK" -F'\t' -v state="$STATE" '
  function intest(file, line,   i) {
    for (i = 1; i <= nt; i++) if (tf[i] == file && line >= ts[i] && line <= te[i]) return 1
    return 0
  }
  function text(file, s, e,   l, t) {
    t = ""
    for (l = s; l <= e; l++) if ((file, l) in code) t = t " " code[file, l]
    return t
  }
  # reach_names(t, from): edges from node `from` (or the roots when empty) to
  # every node whose name is an identifier in t.
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
    rest = substr(rest, p + 1); p = index(rest, ":")
    code[file, substr(rest, 1, p - 1) + 0] = substr(rest, p + 1)
    next
  }
  $1 == "uifree-test-item" { nt++; tf[nt] = $2; ts[nt] = $3; te[nt] = $4; next }
  $1 == "core-type" { nn++; nf[nn] = $2; ns[nn] = $3; ne[nn] = $4; nname[nn] = $5; next }
  $1 == "core-impl-fn" { nm++; mf[nm] = $2; ms[nm] = $3; me[nm] = $4; mself[nm] = $5; mfn[nm] = $6; next }
  $1 == "document-state" { ds = $3; de = $4; next }
  $1 == "R" { nr++; rname[nr] = $2; next }
  END {
    for (i = 1; i <= nn; i++) {
      if (intest(nf[i], ns[i])) continue
      id = nf[i] " " nname[i]
      if (!(id in node)) count++
      node[id] = i
      byname[nname[i]] = byname[nname[i]] SUBSEP id
    }
    for (id in node) { i = node[id]; reach_names(text(nf[i], ns[i], ne[i]), id) }
    for (i = 1; i <= nm; i++) {
      if (intest(mf[i], ms[i])) continue
      self = mself[i]
      sub(/<.*$/, "", self); sub(/^.*[[:space:]&:]/, "", self)
      if (!(self in byname)) continue
      # From `fn name` (an impl header may share its line) to the body.
      sig = text(mf[i], ms[i], me[i])
      if (match(sig, "(^|[^A-Za-z0-9_])fn[[:space:]]+" mfn[i] "([^A-Za-z0-9_]|$)")) sig = substr(sig, RSTART)
      sub(/[{;].*$/, "", sig)
      n = split(byname[self], ids, SUBSEP)
      for (k = 2; k <= n; k++) reach_names(sig, ids[k])
    }
    reach_names(text(state, ds, de), "")
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

while IFS= read -r name; do
  [[ -z "$name" ]] && continue
  echo "Core closure: FAIL — $ROOTS names $name, which is no type in app/core/."
  status=1
done < <({ grep '^U' <<<"$result" || true; } | cut -f2)

viols="$({ grep '^V' <<<"$result" || true; } | cut -f2 | LC_ALL=C sort)"
nodes="$(grep '^C' <<<"$result" | cut -f2)"

if [[ "${1:-}" == "--generate" ]]; then
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
known="$({ grep -vE '^[[:space:]]*(#|$)' "$BASELINE" || true; } | tr -d '\r' | LC_ALL=C sort)"
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

if (( status )); then
  exit 1
fi
echo "Core closure: OK — $nodes core types, $(grep -c . <<<"$known" || true) unreachable on the baseline."
