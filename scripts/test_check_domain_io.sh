#!/usr/bin/env bash
#
# Controls for scripts/check_domain_io.sh: canned clippy JSON against a canned
# allowlist, so each ratchet rule is watched failing and passing without a build.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_domain_io.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

# hit <file> <line> <method>: one clippy disallowed_methods message.
hit() {
  # shellcheck disable=SC2016 # the backticks are clippy's, not a command
  printf '{"reason":"compiler-message","message":{"code":{"code":"clippy::disallowed_methods"},"message":"use of a disallowed method `%s`","spans":[{"is_primary":true,"file_name":"%s","line_start":%s,"column_start":5}]}}\n' "$3" "$1" "$2"
}

# run_case <name> <expected exit> <allowlist body> <clippy JSON lines>
run_case() {
  local name="$1" expect="$2" list="$3" json="$4" status=0
  printf '%s\n' "$list" > "$TMP/$name.list"
  printf '%s\n' "$json" > "$TMP/$name.json"
  DOMAIN_IO_ALLOWLIST="$TMP/$name.list" DOMAIN_IO_CLIPPY_JSON="$TMP/$name.json" \
    bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ $status -ne $expect ]]; then
    echo "FAIL  $name — expected exit $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

A=crates/signex-types/src/a.rs
ENG=crates/signex-engine/src/b.rs
two="$(hit $A 1 std::fs::read; hit $A 9 std::time::Instant::now)"

run_case clean 0 '' '{"reason":"build-finished","success":true}'
run_case unlisted_hit 1 '' "$(hit $A 1 std::fs::read)"
run_case listed_exact 0 "$A 2  # reason" "$two"
run_case listed_grew 1 "$A 1  # reason" "$two"
run_case listed_shrank 1 "$A 3  # reason" "$two"
run_case stale_entry 1 "$A 1  # reason" ''
run_case duplicate_site_counts_once 0 "$A 1  # reason" "$(hit $A 1 std::fs::read; hit $A 1 std::fs::read)"
run_case engine_uuid_left_to_engine_gate 0 '' "$(hit $ENG 3 uuid::Uuid::now_v7)"
run_case engine_fs_counted 1 '' "$(hit $ENG 3 std::fs::read)"
run_case comments_ignored 0 "# $A 5
$A 1  # reason" "$(hit $A 1 std::path::Path::exists)"

# A clippy run that fails (here: no output at all) is an error, never a pass.
status=0
DOMAIN_IO_CLIPPY_JSON="$TMP/missing.json" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  clippy_error — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    clippy_error"
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All domain IO gate controls passed."
