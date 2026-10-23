#!/usr/bin/env bash
#
# Controls for scripts/changelog_assemble.sh: ordering, unknown slug, --check,
# README exclusion, release cut and idempotence.
#
# Usage: scripts/test_changelog_assemble.sh
#
# ok/bad both exit 0, so `A && ok || bad` is a genuine if/else here.
# shellcheck disable=SC2015
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/scripts/changelog_assemble.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

ok() { echo "ok    $1"; }
bad() { echo "FAIL  $1"; failures=$((failures + 1)); }

# fresh_tree <name>: a root with a small CHANGELOG and an empty changelog.d/
fresh_tree() {
  local t="$TMP/$1"
  mkdir -p "$t/changelog.d"
  touch "$t/changelog.d/.gitkeep"
  printf '# Changelog\n\n## [Unreleased]\n\n### Added\n\n- **old add** — kept.\n\n### Fixed\n\n- **old fix** — kept.\n\n## [0.1.0] — 2026-01-01\n\n### Added\n\n- **first** — release.\n' >"$t/CHANGELOG.md"
  printf '%s' "$t"
}

run() { CHANGELOG_ROOT="$1" bash "$TOOL" "${@:2}"; }

# --- assemble: two sections, ordering, new block, README untouched ----------
t="$(fresh_tree assemble)"
printf '%s\n' '- **fix b (#2)** — second.' >"$t/changelog.d/2.fixed.md"
printf '%s\n' '- **add a (#1)** — first.' >"$t/changelog.d/1.added.md"
printf '%s\n' '- **fix c (#2)** — same number.' >"$t/changelog.d/2.fixed.2.md"
printf '%s\n' '- **chg d (#3)** — new block.' >"$t/changelog.d/3.changed.md"
printf '# readme, not a fragment\n' >"$t/changelog.d/README.md"
run "$t" >/dev/null
out="$(cat "$t/CHANGELOG.md")"
unrel="$(printf '%s\n' "$out" | awk '/^## \[0.1.0\]/{exit} {print}')"
want_order="old add
add a
old fix
fix b
fix c
chg d"
got_order="$(printf '%s\n' "$unrel" | grep -o -E 'old add|add a|old fix|fix b|fix c|chg d')"
if [ "$got_order" = "$want_order" ]; then ok "assemble ordering"; else bad "assemble ordering: $got_order"; fi
printf '%s\n' "$unrel" | grep -q '^### Changed$' && ok "new Changed block" || bad "new Changed block"
printf '%s\n' "$out" | grep -q '^## \[0.1.0\] — 2026-01-01$' && ok "release section intact" || bad "release section intact"
if [ ! -e "$t/changelog.d/1.added.md" ] && [ -e "$t/changelog.d/README.md" ] && [ -e "$t/changelog.d/.gitkeep" ]; then
  ok "consumed deleted, README/.gitkeep kept"
else
  bad "consumed deleted, README/.gitkeep kept"
fi

# --- idempotence -------------------------------------------------------------
before="$(cat "$t/CHANGELOG.md")"
run "$t" >/dev/null
[ "$before" = "$(cat "$t/CHANGELOG.md")" ] && ok "second run is a no-op" || bad "second run is a no-op"

# --- --dry-run writes nothing ------------------------------------------------
t="$(fresh_tree dry)"
printf '%s\n' '- **x** — y.' >"$t/changelog.d/9.added.md"
before="$(cat "$t/CHANGELOG.md")"
run "$t" --dry-run | grep -q '^+- \*\*x\*\*' && ok "dry-run prints diff" || bad "dry-run prints diff"
[ "$before" = "$(cat "$t/CHANGELOG.md")" ] && [ -e "$t/changelog.d/9.added.md" ] && ok "dry-run changes nothing" || bad "dry-run changes nothing"

# --- unknown / malformed names fail loudly, nothing written ------------------
for name in 5.bogus.md 5.added.txt added.md 5.added.x.md; do
  t="$(fresh_tree "bad_$name")"
  printf '%s\n' '- **x** — y.' >"$t/changelog.d/$name"
  before="$(cat "$t/CHANGELOG.md")"
  if run "$t" >/dev/null 2>&1; then bad "reject $name"; else
    [ "$before" = "$(cat "$t/CHANGELOG.md")" ] && ok "reject $name" || bad "reject $name wrote"
  fi
  if run "$t" --check >/dev/null 2>&1; then bad "--check rejects $name"; else ok "--check rejects $name"; fi
done

# --- body rules ---------------------------------------------------------------
t="$(fresh_tree body)"
printf '%s\n' 'no bullet here' >"$t/changelog.d/5.added.md"
run "$t" --check >/dev/null 2>&1 && bad "--check rejects non-bullet body" || ok "--check rejects non-bullet body"

# --- --check passes on valid + empty ------------------------------------------
t="$(fresh_tree valid)"
run "$t" --check >/dev/null && ok "--check passes on empty dir" || bad "--check passes on empty dir"
printf '%s\n' '- **x** — y.' >"$t/changelog.d/5.fixed.md"
run "$t" --check >/dev/null && ok "--check passes on valid fragment" || bad "--check passes on valid fragment"
[ -e "$t/changelog.d/5.fixed.md" ] && ok "--check writes nothing" || bad "--check writes nothing"

# --- release cut ---------------------------------------------------------------
t="$(fresh_tree release)"
printf '%s\n' '- **new (#7)** — fresh.' >"$t/changelog.d/7.added.md"
run "$t" --version 0.2.0 --date 2026-02-02 >/dev/null
out="$(cat "$t/CHANGELOG.md")"
printf '%s\n' "$out" | awk '/^## \[Unreleased\]/{u=NR} /^## \[0.2.0\] — 2026-02-02$/{v=NR} /^- \*\*new/{n=NR} /^- \*\*old add/{o=NR} /^## \[0.1.0\]/{r=NR} END{exit !(u<v && v<n && v<o && o<r)}' &&
  ok "release heading opens under Unreleased" || bad "release heading opens under Unreleased"
run "$t" --version 0.2.0 --date 2026-02-02 >/dev/null 2>&1 && bad "duplicate version rejected" || ok "duplicate version rejected"
run "$t" --version 0.3 --date 2026-02-02 >/dev/null 2>&1 && bad "bad version rejected" || ok "bad version rejected"

if [ "$failures" -ne 0 ]; then
  echo "$failures control(s) failed" >&2
  exit 1
fi
echo "all changelog_assemble controls passed"
