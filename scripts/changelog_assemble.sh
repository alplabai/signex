#!/usr/bin/env bash
#
# Fold changelog fragments (changelog.d/<n>.<section>[.<k>].md) into CHANGELOG.md.
# Naming rule and rationale: changelog.d/README.md (GH #748).
#
# Usage:
#   scripts/changelog_assemble.sh                 # fold into ## [Unreleased], delete fragments
#   scripts/changelog_assemble.sh --dry-run       # print the diff, change nothing
#   scripts/changelog_assemble.sh --check         # validate fragment names/bodies only
#   scripts/changelog_assemble.sh --version X.Y.Z --date YYYY-MM-DD
#       # release cut: fold, then open `## [X.Y.Z] — DATE` under Unreleased so the
#       # whole former Unreleased body becomes that release's section
#
# Env: CHANGELOG_ROOT overrides the repo root (used by the test script).
# Portable to macOS bash 3.2 and GNU/Linux (no mapfile, no assoc arrays).
#
set -euo pipefail

ROOT="${CHANGELOG_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
FRAG_DIR="$ROOT/changelog.d"
CHANGELOG="$ROOT/CHANGELOG.md"
# Keep a Changelog order; the "### Heading" is the capitalised slug.
SECTIONS="added changed deprecated removed fixed security"

mode=assemble
version=""
date=""
while [ $# -gt 0 ]; do
  case "$1" in
    --check) mode=check ;;
    --dry-run) mode=dry ;;
    --version) version="${2:-}"; shift ;;
    --date) date="${2:-}"; shift ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "error: unknown argument '$1'" >&2; exit 2 ;;
  esac
  shift
done

if [ -n "$version$date" ]; then
  [ "$mode" = assemble ] || { echo "error: --version/--date cannot combine with --check/--dry-run" >&2; exit 2; }
  printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$' ||
    { echo "error: --version needs X.Y.Z" >&2; exit 2; }
  printf '%s' "$date" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' ||
    { echo "error: --date needs YYYY-MM-DD" >&2; exit 2; }
fi

# Fragment names, ordered by number, then disambiguator. README.md and
# .gitkeep are the only non-fragment files allowed in changelog.d/.
errors=0
names=""
if [ -d "$FRAG_DIR" ]; then
  names="$(cd "$FRAG_DIR" && find . -maxdepth 1 -type f ! -name '.*' | sed 's|^\./||' |
    grep -v -x -e README.md -e .gitkeep |
    sort -t. -k1,1n -k3,3n || true)"
fi
slug_re="$(printf '%s' "$SECTIONS" | tr ' ' '|')"
valid=""
OLDIFS="$IFS"
IFS='
'
for f in $names; do
  if ! printf '%s' "$f" | grep -Eq "^[0-9]+\\.($slug_re)(\\.[0-9]+)?\\.md\$"; then
    echo "error: changelog.d/$f is not <number>.<$slug_re>[.<k>].md" >&2
    errors=$((errors + 1))
    continue
  fi
  body="$FRAG_DIR/$f"
  first="$(grep -v '^[[:space:]]*$' "$body" | head -n 1 || true)"
  case "$first" in
    "- "*) ;;
    *)
      echo "error: changelog.d/$f must start with a '- ' bullet" >&2
      errors=$((errors + 1))
      continue
      ;;
  esac
  if grep -q '^#' "$body"; then
    echo "error: changelog.d/$f must not contain headings" >&2
    errors=$((errors + 1))
    continue
  fi
  valid="$valid$f
"
done
IFS="$OLDIFS"

[ "$errors" -eq 0 ] || exit 1
count="$(printf '%s' "$valid" | grep -c . || true)"
if [ "$mode" = check ]; then
  echo "OK: changelog fragments valid ($count found)"
  exit 0
fi
if [ -z "$valid" ] && [ -z "$version" ]; then
  echo "no changelog fragments to assemble"
  exit 0
fi

[ -f "$CHANGELOG" ] || { echo "error: $CHANGELOG missing" >&2; exit 1; }
grep -q '^## \[Unreleased\]' "$CHANGELOG" || { echo "error: no '## [Unreleased]' heading" >&2; exit 1; }
if [ -n "$version" ]; then
  esc="$(printf '%s' "$version" | sed 's/\./\\./g')"
  if grep -Eq "^## \\[?$esc([]]|[[:space:]]|\$)" "$CHANGELOG"; then
    echo "error: CHANGELOG.md already has a section for $version" >&2
    exit 1
  fi
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
IFS='
'
for f in $valid; do
  slug="$(printf '%s' "$f" | cut -d. -f2)"
  # Drop trailing blank lines so bullets from different fragments stay adjacent.
  tr -d '\r' <"$FRAG_DIR/$f" | awk 'NF{for(;b>0;b--)print ""; print; next}{b++}' >>"$tmp/$slug"
done
IFS="$OLDIFS"

# Per section: append to the first existing "### <Type>" block of Unreleased,
# or open a new block at the end of Unreleased.
awk -v dir="$tmp" -v sections="$SECTIONS" -v ver="$version" -v date="$date" '
function slurp(path,   line, s) {
  s = ""
  while ((getline line < path) > 0) s = s line "\n"
  close(path)
  return s
}
function heading(slug) { return "### " toupper(substr(slug, 1, 1)) substr(slug, 2) }
function flush(   i, e, k, text, tail, nsec) {
  while (n > 0 && L[n] ~ /^[[:space:]]*$/) n--
  nsec = split(sections, S, " ")
  tail = ""
  for (k = 1; k <= nsec; k++) {
    text = slurp(dir "/" S[k])
    if (text == "") continue
    for (i = 1; i <= n; i++) if (L[i] == heading(S[k])) break
    if (i <= n) {
      for (e = i; e < n && L[e + 1] !~ /^##/; e++) ;
      while (L[e] ~ /^[[:space:]]*$/) e--
      ins[e] = ins[e] text
    } else {
      tail = tail "\n" heading(S[k]) "\n\n" text
    }
  }
  for (i = 1; i <= n; i++) {
    print L[i]
    if (i in ins) printf "%s", ins[i]
  }
  printf "%s", tail
  print ""
}
/^## \[Unreleased\]/ && !done {
  print
  if (ver != "") print "\n## [" ver "] — " date
  inrel = 1; n = 0; next
}
inrel && /^## / { flush(); inrel = 0; done = 1 }
inrel { L[++n] = $0; next }
{ print }
END { if (inrel) flush() }
' "$CHANGELOG" >"$tmp/CHANGELOG.new"

if [ "$mode" = dry ]; then
  diff -u "$CHANGELOG" "$tmp/CHANGELOG.new" || true
  exit 0
fi

cp "$tmp/CHANGELOG.new" "$CHANGELOG.tmp.$$"
mv "$CHANGELOG.tmp.$$" "$CHANGELOG"
IFS='
'
for f in $valid; do rm -f "$FRAG_DIR/$f"; done
IFS="$OLDIFS"
echo "assembled $count fragment(s) into CHANGELOG.md"
if [ -n "$version" ]; then
  echo "reminder: add the release headline, install block and the closing"
  echo "  [Full changelog](https://github.com/alplabai/signex/compare/v<prev>...v$version) line by hand."
fi
