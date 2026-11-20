#!/usr/bin/env bash
#
# Controls for scripts/check_app_catch_unwind.sh: each shape the gate must
# catch fails it, and each shape it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_app_catch_unwind.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

# run_case <name> <expected: fail|pass> <file under app> <body> <baseline>
run_case() {
  local name="$1" expect="$2" file="$3" body="$4" baseline="$5" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/crates/signex-app/src/app/$(dirname "$file")" "$tree/scripts"
  printf '%s\n' "$body" > "$tree/crates/signex-app/src/app/$file"
  printf '%s\n' "$baseline" > "$tree/scripts/app-catch-unwind-baseline.txt"
  status=0
  APP_CATCH_UNWIND_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

run_case path_call fail ui.rs 'fn f() { std::panic::catch_unwind(|| 1); }' 0
run_case bare_call fail ui.rs 'fn f() { catch_unwind(AssertUnwindSafe(|| 1)); }' 0
run_case at_baseline pass ui.rs 'fn f() { catch_unwind(|| 1); }' 1
run_case below_baseline fail ui.rs 'fn f() {}' 1
run_case test_mod pass ui.rs 'fn f() {}
#[cfg(test)]
// why
mod tests { fn t() { std::panic::catch_unwind(|| 1); } }' 0
run_case test_fn pass ui.rs '#[cfg(test)]
fn t() { catch_unwind(|| 1); }' 0
run_case tests_file pass save_tests.rs 'fn t() { catch_unwind(|| 1); }' 0
run_case tests_dir pass agent/tests/client.rs 'fn t() { catch_unwind(|| 1); }' 0
run_case future_method pass ui.rs 'fn f() { fut.catch_unwind(); }' 0

# A tree without the app sources is an error, never a pass.
mkdir -p "$TMP/no_app/scripts"
echo 0 > "$TMP/no_app/scripts/app-catch-unwind-baseline.txt"
status=0
APP_CATCH_UNWIND_ROOT="$TMP/no_app" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_app — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_app"
fi

if [[ $failures -ne 0 ]]; then
  echo "check_app_catch_unwind controls: $failures case(s) failed"
  exit 1
fi
echo "check_app_catch_unwind controls: OK"
