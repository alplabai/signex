#!/usr/bin/env bash
# Runs the schematic render-path benchmark (#2040), one process per
# path / MSAA / view so the memory figures are not mixed between runs.
# Needs a GPU. Output: one `BENCH` line per measurement, in $OUT.
#
#   bash scripts/bench_schematic_render.sh [out-file]
#
# SIGNEX_BENCH_SIZE (small|medium|large), SIGNEX_BENCH_DPR and
# SIGNEX_BENCH_VARIANT (full|norot|notext) pass through. Method and results:
# docs/architecture/schematic-render-bench.md.
set -uo pipefail

OUT="${1:-bench-schematic-render.log}"
: >"$OUT"
TMP="$(mktemp)"
FAILED=0
trap 'rm -f "$TMP"' EXIT

run() { # path aa view; one retry, a GPU watchdog reset can kill a run
    local attempt
    for attempt in 1 2; do
        echo "== path=$1 aa=$2 view=$3 variant=${SIGNEX_BENCH_VARIANT:-full} dpr=${SIGNEX_BENCH_DPR:-1} size=${SIGNEX_BENCH_SIZE:-large} attempt=$attempt" >>"$OUT"
        SIGNEX_BENCH_PATHS="$1" SIGNEX_BENCH_AA="$2" SIGNEX_BENCH_VIEWS="$3" \
            cargo test -p signex-app --release --lib render_bench -- \
            --ignored --nocapture --test-threads=1 >"$TMP" 2>&1
        local status=$?
        grep -E 'BENCH|panicked|PANIC|Validation|Metal' "$TMP" >>"$OUT"
        [ "$status" -eq 0 ] && return 0
    done
    echo "FAILED path=$1 aa=$2 view=$3" >>"$OUT"
    FAILED=1
}

# SIGNEX_BENCH_AAS="off msaa4" and SIGNEX_BENCH_AUX=1 (the single layers of the
# GPU mount, which say where its time goes) narrow or widen the run.
for view in fit work; do
    for aa in ${SIGNEX_BENCH_AAS:-off msaa4}; do
        for path in canvas gpu; do run "$path" "$aa" "$view"; done
    done
    if [ "${SIGNEX_BENCH_AUX:-1}" = 1 ]; then
        for path in backdrop shaders texts overlay; do run "$path" off "$view"; done
    fi
done

exit "$FAILED"
