#!/usr/bin/env bash
#
# Controls for scripts/check_arch_guard.sh: each shape the gate claims to catch
# must fail it, and each shape it must leave alone must pass. Runs the gate on
# hand-written `cargo metadata` JSON, so no cargo or network is needed.
#
# Usage: scripts/test_check_arch_guard.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_arch_guard.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

# run_case <name> <expected: fail|pass> <edges file body> <packages JSON array>
run_case() {
  local name="$1" expect="$2" edges="$3" packages="$4" status
  printf '%s\n' "$edges" > "$TMP/$name.edges"
  printf '{"packages": %s}\n' "$packages" > "$TMP/$name.json"
  status=0
  ARCH_GUARD_EDGES="$TMP/$name.edges" ARCH_GUARD_METADATA="$TMP/$name.json" \
    bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

dep() { printf '{"name": "%s", "kind": %s}' "$1" "${2:-null}"; }

TYPES='{"name": "signex-types", "dependencies": []}'
NET_OK="{\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types)]}"

# --- negative controls: the gate must pass these --------------------------

run_case matching_table pass \
  'signex-net -> signex-types' \
  "[$TYPES, $NET_OK]"

run_case trailing_comment_and_blank_lines pass \
  '# header

signex-net -> signex-types  # why this edge exists' \
  "[$TYPES, $NET_OK]"

run_case dev_dependency_is_exempt pass \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep iced '"dev"'), $(dep signex-app '"dev"')]}, {\"name\": \"signex-app\", \"dependencies\": []}]"

run_case external_non_ui_dep_is_fine pass \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep serde)]}]"

run_case app_may_use_iced pass \
  'signex-app -> signex-types' \
  "[$TYPES, {\"name\": \"signex-app\", \"dependencies\": [$(dep signex-types), $(dep iced), $(dep wgpu)]}]"

# --- positive controls: the gate must flag these ---------------------------

run_case new_edge_not_in_table fail \
  'signex-net -> signex-types' \
  "[$TYPES, $NET_OK, {\"name\": \"signex-erc\", \"dependencies\": [$(dep signex-net)]}]"

run_case table_lists_a_removed_edge fail \
  'signex-net -> signex-types
signex-erc -> signex-net' \
  "[$TYPES, $NET_OK, {\"name\": \"signex-erc\", \"dependencies\": []}]"

run_case domain_crate_on_iced fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep iced)]}]"

run_case domain_crate_on_iced_prefixed fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep iced_aw)]}]"

run_case domain_crate_on_wgpu fail \
  '# no workspace edges' \
  "[{\"name\": \"signex-types\", \"dependencies\": [$(dep wgpu)]}]"

run_case domain_crate_on_widgets fail \
  'signex-library -> signex-widgets' \
  "[{\"name\": \"signex-widgets\", \"dependencies\": []}, {\"name\": \"signex-library\", \"dependencies\": [$(dep signex-widgets)]}]"

# --- and the real tree passes ----------------------------------------------

if bash "$GATE" >/dev/null 2>&1; then
  echo "ok    real_tree"
else
  echo "FAIL  real_tree — the gate rejects the current workspace"
  failures=$((failures + 1))
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All arch-guard controls passed."
