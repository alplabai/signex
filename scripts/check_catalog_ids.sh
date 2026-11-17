#!/usr/bin/env bash
#
# Command-id deletion lock (#2447): a command id in the catalog may be added
# but never silently removed. User keymap profiles, agent scripts and the
# `signex commands` output name commands by id, so a lost or renamed id
# breaks them.
#
# scripts/catalog-ids.txt lists every accepted id, one per line, sorted with
# LC_ALL=C. The gate reads the real ids out of
# crates/signex-commands/src/catalog with ast-grep (scripts/catalog-command-id.yml):
# the `id:` of each `CommandMetadata { .. }` row, outside `#[cfg(test)]`
# modules. Comments are not code, so a commented-out row counts as removed.
#
# - A listed id missing from the catalog fails: it was deleted or renamed.
#   Removing a command on purpose means removing its line here in the same
#   PR, which CODEOWNERS routes to the maintainers.
# - A catalog id missing from the list fails: add its line, so a later
#   deletion of the new command is caught too.
# - An id defined twice fails: the first row would shadow the second.
#
# Usage: scripts/check_catalog_ids.sh   (CATALOG_IDS_ROOT overrides the tree,
# for scripts/test_check_catalog_ids.sh)
set -euo pipefail

SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
ROOT="${CATALOG_IDS_ROOT:-$(cd "$SCRIPTS/.." && pwd)}"
SRC="crates/signex-commands/src/catalog"
LIST="scripts/catalog-ids.txt"
RULE="$SCRIPTS/catalog-command-id.yml"

cd "$ROOT"
if [[ ! -d "$SRC" ]]; then
  echo "Command-id gate: FAIL — no $SRC under $ROOT."
  exit 2
fi
if [[ ! -f "$LIST" ]]; then
  echo "Command-id gate: FAIL — no $LIST under $ROOT."
  exit 2
fi
if ! command -v ast-grep >/dev/null; then
  echo "Command-id gate: FAIL — ast-grep is not installed."
  exit 2
fi

found="$(ast-grep scan --rule "$RULE" --json "$SRC" | jq -r '.[].text' | LC_ALL=C sort)"
listed="$(grep -v -e '^#' -e '^[[:space:]]*$' "$LIST" | LC_ALL=C sort)"

if [[ -z "$found" ]]; then
  echo "Command-id gate: FAIL — no command ids found under $SRC; the rule no longer matches the catalog."
  exit 2
fi

status=0
duplicates="$(uniq -d <<<"$found")"
if [[ -n "$duplicates" ]]; then
  echo "Command-id gate: FAIL — ids defined more than once:"
  sed 's/^/  /' <<<"$duplicates"
  status=1
fi

missing="$(LC_ALL=C comm -23 <(uniq <<<"$listed") <(uniq <<<"$found"))"
if [[ -n "$missing" ]]; then
  echo "Command-id gate: FAIL — listed in $LIST but gone from the catalog (deleted or renamed):"
  sed 's/^/  /' <<<"$missing"
  echo "  Removing a command needs a maintainer decision: drop its line from $LIST in"
  echo "  the same PR and name the removal under 'Behaviour changes'."
  status=1
fi

unlisted="$(LC_ALL=C comm -13 <(uniq <<<"$listed") <(uniq <<<"$found"))"
if [[ -n "$unlisted" ]]; then
  echo "Command-id gate: FAIL — new ids missing from $LIST (add one line each, sorted):"
  sed 's/^/  /' <<<"$unlisted"
  status=1
fi

if [[ $status -eq 0 ]]; then
  echo "Command-id gate: OK ($(wc -l <<<"$found" | tr -d ' ') ids)."
fi
exit $status
