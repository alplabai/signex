#!/usr/bin/env bash
#
# Controls for scripts/check_catalog_ids.sh: each way of losing a command
# fails the gate, and each shape it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_catalog_ids.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

ROW_A='    CommandMetadata { id: "alpha", label: X, ..CommandMetadata::DEFAULT },'
ROW_B='    CommandMetadata {
        id: "beta",
        category: "edit",
        ..CommandMetadata::DEFAULT
    },'

# run_case <name> <expected exit> <catalog body> <list body>
run_case() {
  local name="$1" expect="$2" body="$3" list="$4" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/crates/signex-commands/src/catalog" "$tree/scripts"
  printf '%s\n' "$body" > "$tree/crates/signex-commands/src/catalog/area.rs"
  printf '%s\n' "$list" > "$tree/scripts/catalog-ids.txt"
  status=0
  CATALOG_IDS_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ $status -ne $expect ]]; then
    echo "FAIL  $name — expected exit $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

table() { printf 'pub(super) const T: &[CommandMetadata] = &[\n%s\n];\n' "$1"; }
LIST_AB='# header comment
alpha
beta'

run_case clean 0 "$(table "$ROW_A
$ROW_B")" "$LIST_AB"
run_case deleted 1 "$(table "$ROW_A")" "$LIST_AB"
run_case commented_out 1 "$(table "$ROW_A
    // CommandMetadata { id: \"beta\", ..CommandMetadata::DEFAULT },")" "$LIST_AB"
run_case renamed 1 "$(table "$ROW_A
    CommandMetadata { id: \"beta_renamed\", ..CommandMetadata::DEFAULT },")" "$LIST_AB"
run_case unlisted_new 1 "$(table "$ROW_A
$ROW_B
    CommandMetadata { id: \"gamma\", ..CommandMetadata::DEFAULT },")" "$LIST_AB"
run_case listed_new 0 "$(table "$ROW_A
$ROW_B
    CommandMetadata { id: \"gamma\", ..CommandMetadata::DEFAULT },")" "$LIST_AB
gamma"
run_case list_unsorted 1 "$(table "$ROW_A
$ROW_B")" 'beta
alpha'
run_case list_repeated_line 1 "$(table "$ROW_A
$ROW_B")" "$LIST_AB
beta"
run_case duplicate 1 "$(table "$ROW_A
$ROW_A
$ROW_B")" "$LIST_AB"
run_case other_struct_ignored 0 "$(table "$ROW_A
$ROW_B")
const O: Other = Other { id: \"not_a_command\" };" "$LIST_AB"
run_case test_module_ignored 0 "$(table "$ROW_A
$ROW_B")
#[cfg(test)]
mod tests {
    const F: CommandMetadata = CommandMetadata { id: \"test_only\", ..CommandMetadata::DEFAULT };
}" "$LIST_AB"
run_case only_in_test_module 1 "$(table "$ROW_A")
#[cfg(test)]
mod tests {
    const F: CommandMetadata = CommandMetadata { id: \"beta\", ..CommandMetadata::DEFAULT };
}" "$LIST_AB"

# The rule matching nothing is an error, never a pass.
run_case no_rows 2 'pub(super) const T: &[CommandMetadata] = &[];' "$LIST_AB"

# A tree without the catalog or the list is an error, never a pass.
for missing in no_catalog no_list; do
  tree="$TMP/$missing"
  mkdir -p "$tree/scripts" "$tree/crates/signex-commands/src/catalog"
  if [[ $missing == no_catalog ]]; then
    rm -rf "$tree/crates"
    printf 'alpha\n' > "$tree/scripts/catalog-ids.txt"
  else
    table "$ROW_A" > "$tree/crates/signex-commands/src/catalog/area.rs"
  fi
  status=0
  CATALOG_IDS_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ $status -ne 2 ]]; then
    echo "FAIL  $missing — expected exit 2, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $missing"
  fi
done

if [[ $failures -ne 0 ]]; then
  echo "check_catalog_ids controls: $failures case(s) failed"
  exit 1
fi
echo "check_catalog_ids controls: OK"
