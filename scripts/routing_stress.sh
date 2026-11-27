#!/usr/bin/env bash
#
# Routing stress run (#2967): the release check of the PCB Routing exit
# criteria. Builds the routing tests in release, soaks every board (10 000
# seeded actions each by default) and then runs the bench alone, one test
# thread, with the latency thresholds asserted. Writes to the output
# directory:
#
#   routing-stress.json   the stress report, one entry per board
#   routing-report.txt    the report lines and the summary table
#   soak.log, bench.log   the full test output
#
# Attach routing-stress.json and routing-report.txt to the release-prep PR.
# A failure prints the line that replays it; the same seed, board and
# action count replay it exactly (--seed, --board, --actions here).
# What it checks and the report format: docs/PERFORMANCE.md, "Router soak
# and bench". The boards are in Git LFS: run `git lfs pull` first; a board
# left as a pointer fails the run.
#
# Usage: scripts/routing_stress.sh [--seed N] [--actions N] [--board NAME]
#                                  [--out DIR] [--no-bench]
set -euo pipefail

usage() {
  sed -n '20,21p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

seed=""
actions=""
board=""
out="target/routing-stress"
bench=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --seed) seed="${2:?--seed needs a value}"; shift 2 ;;
    --actions) actions="${2:?--actions needs a value}"; shift 2 ;;
    --board) board="${2:?--board needs a value}"; shift 2 ;;
    --out) out="${2:?--out needs a value}"; shift 2 ;;
    --no-bench) bench=0; shift ;;
    -h | --help) usage ;;
    *) echo "routing_stress: unknown argument '$1'" >&2; usage ;;
  esac
done

cd "$(dirname "$0")/.."
mkdir -p "$out"
out="$(cd "$out" && pwd)"
rm -f "$out/routing-stress.json" "$out/routing-report.txt"

export SIGNEX_SOAK_SEED="$seed"
export SIGNEX_SOAK_ACTIONS="$actions"
export SIGNEX_SOAK_BOARD="$board"
export SIGNEX_REQUIRE_ROUTING_CORPUS=1
export SIGNEX_ROUTING_REPORT="$out/routing-report.txt"
export SIGNEX_ROUTING_REPORT_JSON="$out/routing-stress.json"

cargo test --release -p signex-engine --test routing --no-run

status=0
cargo test --release -p signex-engine --test routing soak_long -- --ignored --nocapture \
  2>&1 | tee "$out/soak.log" || status=1

if [[ $bench -eq 1 ]]; then
  SIGNEX_ROUTING_ASSERT_LATENCY=1 \
    cargo test --release -p signex-engine --test routing bench_thresholds -- \
    --ignored --nocapture --test-threads 1 2>&1 | tee "$out/bench.log" || status=1
fi

echo
echo "Routing stress report: $out/routing-stress.json"
echo "Report lines and summary table: $out/routing-report.txt"
if [[ $status -ne 0 ]]; then
  echo "Routing stress run: FAILED (the replay lines are in $out/soak.log and $out/bench.log)"
fi
exit "$status"
