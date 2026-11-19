#!/usr/bin/env bash
#
# Routing corpus gate (#2481): testdata/routing-corpus/ holds third-party
# boards under their own licences, each listed in MANIFEST.tsv. The gate
# fails when a board has no manifest row or a row has no board, when a row
# names a licence outside the allow-list, when a licence text or notice the
# manifest points to is missing, when there are fewer than 50 boards, and
# when a file outside testdata/ and the routing tests reads the corpus.
# Why the allow-list is what it is: testdata/routing-corpus/README.md; the
# use rule: docs/LICENSING.md, "Third-party test data".
#
# Portable: bash 3.2 (macOS) and POSIX tools only, no GNU-only flags.
#
# Usage: scripts/check_routing_corpus.sh   (ROUTING_CORPUS_ROOT overrides the
# tree, for scripts/test_check_routing_corpus.sh)
set -euo pipefail

ROOT="${ROUTING_CORPUS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DIR="testdata/routing-corpus"
MANIFEST="$DIR/MANIFEST.tsv"
MIN_BOARDS=50
TAB="$(printf '\t')"
HEADER="slug${TAB}upstream${TAB}commit${TAB}source_path${TAB}spdx${TAB}license_source${TAB}license_text${TAB}notice${TAB}attribution${TAB}notes${TAB}converter${TAB}loss_report${TAB}layers${TAB}size_class"
COLUMNS=14

# The licences a corpus board may carry. MIT needs its notice (the upstream
# copyright line) shipped with the board; the ULX3S text is the upstream
# file itself, copyright line included.
ALLOWED="MIT LicenseRef-MIT-ULX3S CC-BY-3.0 CC-BY-4.0 CERN-OHL-P-2.0 CERN-OHL-S-2.0 CERN-OHL-W-2.0 CERN-OHL-1.2"
NEEDS_NOTICE="MIT"

cd "$ROOT"
if [[ ! -f "$MANIFEST" ]]; then
  echo "Routing corpus gate: FAIL — no $MANIFEST under $ROOT."
  exit 2
fi

errors=0
fail() {
  echo "  $1"
  errors=$((errors + 1))
}

in_list() {
  local word="$1" list="$2" item
  for item in $list; do
    [[ "$item" == "$word" ]] && return 0
  done
  return 1
}

# Slugs seen so far, one "slug<TAB>line" per line, newline-led (no
# associative arrays: macOS ships bash 3.2). Read by parameter expansion
# alone, so a lookup forks nothing: `seen_line` sets `previous` to the
# line a slug was first seen on, or to empty.
NL='
'
seen="$NL"
previous=""
seen_line() {
  previous=""
  case "$seen" in
    *"$NL$1$TAB"*)
      local rest="${seen#*"$NL$1$TAB"}"
      previous="${rest%%"$NL"*}"
      ;;
  esac
  return 0
}

first="$(head -n 1 "$MANIFEST" | tr -d '\r')"
if [[ "$first" != "$HEADER" ]]; then
  fail "$MANIFEST: the header must be: $(printf '%s' "$HEADER" | tr '\t' '|')"
fi

line=1
while IFS= read -r raw || [[ -n "$raw" ]]; do
  line=$((line + 1))
  row="${raw%$'\r'}"
  [[ -z "$row" ]] && continue
  IFS="$TAB" read -r -a cells <<< "$row"
  if [[ ${#cells[@]} -ne $COLUMNS ]]; then
    fail "$MANIFEST:$line: ${#cells[@]} columns, expected $COLUMNS"
    continue
  fi
  slug="${cells[0]}" upstream="${cells[1]}" commit="${cells[2]}" spdx="${cells[4]}"
  text="${cells[6]}" notice="${cells[7]}"
  if [[ ! "$slug" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
    fail "$MANIFEST:$line: slug '$slug' must be lower-case letters, digits and dashes"
  fi
  seen_line "$slug"
  if [[ -n "$previous" ]]; then
    fail "$MANIFEST:$line: slug '$slug' repeats line $previous"
  else
    seen="$seen$slug$TAB$line$NL"
  fi
  [[ "$upstream" =~ ^https:// ]] || fail "$MANIFEST:$line: $slug: upstream '$upstream' is not an https URL"
  [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || fail "$MANIFEST:$line: $slug: commit '$commit' is not a full SHA"
  if ! in_list "$spdx" "$ALLOWED"; then
    fail "$MANIFEST:$line: $slug: licence '$spdx' is not on the allow-list ($ALLOWED)"
  fi
  if [[ "$text" != "LICENSES/$spdx.txt" ]]; then
    fail "$MANIFEST:$line: $slug: license_text must be LICENSES/$spdx.txt, not '$text'"
  elif [[ ! -f "$DIR/$text" ]]; then
    fail "$MANIFEST:$line: $slug: $DIR/$text does not exist"
  fi
  if [[ "$notice" == "-" ]]; then
    if in_list "$spdx" "$NEEDS_NOTICE"; then
      fail "$MANIFEST:$line: $slug: $spdx needs its upstream notice under LICENSES/notices/"
    fi
  elif [[ "$notice" != LICENSES/* || ! -f "$DIR/$notice" ]]; then
    fail "$MANIFEST:$line: $slug: notice '$notice' is not a file under $DIR/LICENSES/"
  fi
  [[ -f "$DIR/$slug.snxpcb" ]] || fail "$MANIFEST:$line: $slug: no $DIR/$slug.snxpcb"
done < <(tail -n +2 "$MANIFEST")

boards=0
while IFS= read -r path; do
  boards=$((boards + 1))
  slug="${path##*/}"
  slug="${slug%.snxpcb}"
  seen_line "$slug"
  [[ -n "$previous" ]] || fail "$path has no row in $MANIFEST"
done < <(find "$DIR" -maxdepth 1 -type f -name '*.snxpcb' | sort)

if [[ $boards -lt $MIN_BOARDS ]]; then
  fail "$DIR holds $boards boards; the corpus needs at least $MIN_BOARDS"
fi

# The use rule (docs/LICENSING.md, "Third-party test data"): only the corpus
# itself and the routing tests read the boards, so they cannot drift into an
# example, a template or an output fixture. Prose (*.md), the LFS rule, the
# workflows that fetch the corpus and this gate may name the directory.
MAY_NAME_CORPUS='^(testdata/|crates/signex-engine/tests/routing/|\.gitattributes$|\.github/workflows/(ci|routing-soak)\.yml$|scripts/(test_)?check_routing_corpus\.sh$)|\.md$'

# Every file of the tree: the tracked ones in a checkout, all of them in a
# plain tree (the controls). `.git`, `target` and `node_modules` are skipped.
tree_files() {
  if [[ -e .git ]] && git ls-files >/dev/null 2>&1; then
    git -c core.quotepath=off ls-files
  else
    find . \( -name .git -o -name target -o -name node_modules \) -prune -o -type f -print |
      sed 's|^\./||'
  fi
}

# The files outside the allowed set that name the corpus: one filter, one
# batched grep (-s: a submodule path in the list is a directory).
readers() {
  tree_files | { grep -v -E "$MAY_NAME_CORPUS" || true; } | tr '\n' '\0' |
    { xargs -0 grep -s -I -l -F -e 'routing-corpus' -- || true; } | sort
}

while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  fail "$path reads $DIR/; only testdata/ and crates/signex-engine/tests/routing/ may (docs/LICENSING.md, \"Third-party test data\")"
done < <(readers)

if [[ $errors -ne 0 ]]; then
  echo "Routing corpus gate: FAIL — $errors problem(s) above."
  exit 1
fi
echo "Routing corpus gate: OK — $boards boards, each with a manifest row and an allowed licence; only the routing tests read them."
