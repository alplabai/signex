#!/usr/bin/env bash
#
# Command entry ratchet (#2731): a command's operands run through the entries
# in crates/signex-app/src/app/command (`replay_operands`,
# `run_headless_operands`) or the agent route in app/agent. Zero production
# calls of `run_operands(` or `CommandArgs::parse_operands(` are allowed
# anywhere else in signex-app, so a new front end cannot grow its own route
# past the command gates.
#
# Left out: test files (`tests.rs`, `tests_*.rs`, `*_tests.rs`, anything under
# a `tests/` or `*_tests/` folder) and `src/test_adapter/`, the test harness's
# own front end.
# The signex-cli crate is outside the scan: it only parses operands, then
# runs them through the headless entry.
#
# Blind spots: a line-based read, so a call split across lines after the name
# is still seen but a renamed import is not; `//` inside a string ends the
# code early; a `#[cfg(test)]` block in a production file counts as production
# (it fails safe).
#
# Usage: scripts/check_command_entry.sh   (COMMAND_ENTRY_ROOT overrides the
# tree, for scripts/test_check_command_entry.sh)
set -euo pipefail

ROOT="${COMMAND_ENTRY_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
APP="crates/signex-app/src"

cd "$ROOT"
if [[ ! -d "$APP" ]]; then
  echo "Command entry ratchet: FAIL — no $APP under $ROOT."
  exit 2
fi

status=0
hits="$(grep -rnE --include='*.rs' 'run_operands\(|CommandArgs::parse_operands\(' "$APP")" || status=$?
if (( status > 1 )); then
  echo "Command entry ratchet: FAIL — grep could not scan $APP."
  exit 2
fi

outside="$(printf '%s\n' "$hits" | awk -F: -v app="^$APP/(app/(command|agent)|test_adapter)/" '
  NF < 3 { next }
  $1 ~ app { next }
  $1 ~ /(^|\/|_)tests(\/|\.rs$)/ || $1 ~ /\/tests_[a-z0-9_]+\.rs$/ { next }
  {
    code = $0
    sub(/^[^:]*:[0-9]+:/, "", code)
    sub(/\/\/.*/, "", code)
    if (code ~ /run_operands\(|CommandArgs::parse_operands\(/) print
  }')"

if [[ -n "$outside" ]]; then
  echo "Command entry ratchet: FAIL — operands run outside app/command and app/agent (#2731):"
  printf '%s\n' "$outside" | awk '{ print "  " $0 }'
  echo "  Add or call a sanctioned entry in app/command that states its origin."
  exit 1
fi
echo "Command entry ratchet: OK — operands run only through app/command and app/agent."
