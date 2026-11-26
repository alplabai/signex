#!/usr/bin/env bash
# Runs the 25 000-part library scale bench (#3115): cold mount and
# Components-panel search latency, pinned to one core. Output: one `BENCH`
# line per measurement, in $OUT.
#
#   bash scripts/bench_library_scale.sh [out-file]
#
# SIGNEX_BENCH_ROWS sets the part count (default 25000), SIGNEX_BENCH_CORE the
# core (default 0). Thresholds, method and results:
# docs/architecture/library-scale-bench.md.
set -uo pipefail

OUT="${1:-bench-library-scale.log}"
CORE="${SIGNEX_BENCH_CORE:-0}"
cargo test -p signex-app --release --lib --no-run || exit 1
taskset -c "$CORE" cargo test -p signex-app --release --lib scale_bench -- \
    --ignored --nocapture --test-threads=1 2>&1 | grep -E 'BENCH|panicked|test result' | tee "$OUT"
exit "${PIPESTATUS[0]}"
