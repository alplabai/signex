#!/usr/bin/env bash
#
# Controls for scripts/check_command_entry.sh: each shape the ratchet must
# catch fails it, and each shape it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_command_entry.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

# run_case <name> <expected: fail|pass> <relative file> <body>
run_case() {
  local name="$1" expect="$2" file="$3" body="$4" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/$(dirname "$file")"
  printf '%s\n' "$body" > "$tree/$file"
  status=0
  COMMAND_ENTRY_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

APP=crates/signex-app/src
run_case run_in_handler fail "$APP/app/handlers/log.rs" 'fn f() { app.run_operands(c, &a); }'
run_case parse_in_handler fail "$APP/app/handlers/log.rs" 'fn f() { CommandArgs::parse_operands(c, r); }'
run_case run_in_headless fail "$APP/headless.rs" 'fn f() { app.run_operands(c, &a); }'
run_case after_code_comment fail "$APP/app/menu.rs" 'fn f() { app.run_operands(c, &a); } // why'
run_case in_command pass "$APP/app/command/replay.rs" 'fn f() { app.run_operands(c, &a); }'
run_case in_agent pass "$APP/app/agent/handler.rs" 'fn f() { CommandArgs::parse_operands(c, r); }'
run_case in_test_adapter pass "$APP/test_adapter/mod.rs" 'fn f() { app.run_operands(c, &a); }'
run_case tests_file pass "$APP/app/board/tests.rs" 'fn f() { app.run_operands(c, &a); }'
run_case named_tests_file pass "$APP/app/board/pour_tests.rs" 'fn f() { app.run_operands(c, &a); }'
run_case tests_prefix_file pass "$APP/app/board/tests_print.rs" 'fn f() { app.run_operands(c, &a); }'
run_case tests_like_folder fail "$APP/app/board/tests_helper/run.rs" 'fn f() { app.run_operands(c, &a); }'
run_case tests_folder pass "$APP/app/handlers/tests/log.rs" 'fn f() { app.run_operands(c, &a); }'
run_case seam_tests_folder pass "$APP/app/edit_tail_seam_tests/a.rs" 'fn f() { app.run_operands(c, &a); }'
run_case doc_comment pass "$APP/app/handlers/log.rs" '/// Goes through `run_operands(..)`.'
run_case clean pass "$APP/app/handlers/log.rs" 'fn f() { app.replay_operands(c, m, t); }'

# A tree with no app source is an error, never a pass.
mkdir -p "$TMP/no_app"
status=0
COMMAND_ENTRY_ROOT="$TMP/no_app" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_app — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_app"
fi

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All command entry ratchet controls passed."
