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

# run_case <name> <expected: fail|pass> <edges file body> <packages JSON array> [message]
#
# The exit code alone is not enough: a gate that crashes under `set -e` also
# exits 1, so a "must fail" case would pass without testing its rule. A fail
# case must print `message`; a pass case must print the OK line.
run_case() {
  local name="$1" expect="$2" edges="$3" packages="$4" want="${5:-}" status out
  printf '%s\n' "$edges" > "$TMP/$name.edges"
  printf '{"packages": %s}\n' "$packages" > "$TMP/$name.json"
  status=0
  out="$(ARCH_GUARD_EDGES="$TMP/$name.edges" ARCH_GUARD_METADATA="$TMP/$name.json" \
    bash "$GATE" 2>&1)" || status=$?
  [[ "$expect" == pass ]] && want="Arch guard: OK"
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  elif [[ -n "$want" && "$out" != *"$want"* ]]; then
    echo "FAIL  $name — output lacks '$want': $out"
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
  "[$TYPES, {\"name\": \"signex-app\", \"dependencies\": [$(dep signex-types), $(dep iced), $(dep wgpu)]}]" \
  "domain crate 'signex-types' depends on 'wgpu'"

# --- positive controls: the gate must flag these ---------------------------

run_case new_edge_not_in_table fail \
  'signex-net -> signex-types' \
  "[$TYPES, $NET_OK, {\"name\": \"signex-erc\", \"dependencies\": [$(dep signex-net)]}]" \
  "new workspace edge 'signex-erc -> signex-net'"

run_case table_lists_a_removed_edge fail \
  'signex-net -> signex-types
signex-erc -> signex-net' \
  "[$TYPES, $NET_OK, {\"name\": \"signex-erc\", \"dependencies\": []}]" \
  "no Cargo.toml has any more"

run_case domain_crate_on_iced fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep iced)]}]" \
  "domain crate 'signex-net' depends on 'iced'"

run_case domain_crate_on_iced_prefixed fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep iced_aw)]}]" \
  "domain crate 'signex-net' depends on 'iced_aw'"

run_case domain_crate_on_wgpu fail \
  '# no workspace edges' \
  "[{\"name\": \"signex-types\", \"dependencies\": [$(dep wgpu)]}]"

run_case domain_crate_on_widgets fail \
  'signex-library -> signex-widgets' \
  "[{\"name\": \"signex-widgets\", \"dependencies\": []}, {\"name\": \"signex-library\", \"dependencies\": [$(dep signex-widgets)]}]" \
  "domain crate 'signex-library' depends on 'signex-widgets'"

run_case domain_crate_on_wgpu_subcrate fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep wgpu-types)]}]" \
  "domain crate 'signex-net' depends on 'wgpu-types'"

run_case domain_crate_on_naga fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep naga)]}]" \
  "domain crate 'signex-net' depends on 'naga'"

run_case domain_crate_on_glyphon fail \
  'signex-net -> signex-types' \
  "[$TYPES, {\"name\": \"signex-net\", \"dependencies\": [$(dep signex-types), $(dep glyphon)]}]" \
  "domain crate 'signex-net' depends on 'glyphon'"

# A table holding only comments is an empty table, not a crash.
run_case comment_only_table_with_no_edges pass \
  '# no workspace edges' \
  "[$TYPES]"

# --- --generate keeps the notes recorded on surviving edges -----------------

printf '%s\n' '# header' 'signex-net -> signex-types  # why this edge exists' 'signex-erc -> signex-net' \
  > "$TMP/generate.edges"
printf '{"packages": [%s, %s]}\n' "$TYPES" "$NET_OK" > "$TMP/generate.json"
# No `|| true`: a --generate that crashes must fail this control, not hide
# behind a file it never rewrote.
generate_status=0
ARCH_GUARD_EDGES="$TMP/generate.edges" ARCH_GUARD_METADATA="$TMP/generate.json" \
  bash "$GATE" --generate >/dev/null 2>&1 || generate_status=$?
if [[ "$generate_status" == 0 ]] \
  && grep -qx 'signex-net -> signex-types  # why this edge exists' "$TMP/generate.edges" \
  && ! grep -q 'signex-erc -> signex-net' "$TMP/generate.edges"; then
  echo "ok    generate_keeps_notes_and_drops_stale_edges"
else
  echo "FAIL  generate_keeps_notes_and_drops_stale_edges — got: $(cat "$TMP/generate.edges")"
  failures=$((failures + 1))
fi

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
