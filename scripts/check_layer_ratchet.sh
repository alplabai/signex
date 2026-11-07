#!/usr/bin/env bash
#
# Layer ratchet (#664, ADR-0001 D4/D5). Three counts that may only go down,
# each measured by grep over crates/**/*.rs and compared with the checked-in
# scripts/layer-ratchet-baseline.txt:
#
#   symbol_transform_outside_types  `struct SymbolTransform` outside signex-types
#                                   (the shared kernel owns the type).
#   union_find_outside_net          a union-find implementation outside
#                                   signex-net (signex-net/src/uf.rs owns it).
#   app_library_editor_internals    `library::editor::<x>::{state,canvas,
#                                   updates,views}` named from outside
#                                   crates/signex-app/src/library (slice
#                                   internals reached from the rest of the app).
#
# A count above its baseline fails (new debt). A count below its baseline also
# fails, so the baseline is lowered in the same PR that pays the debt and the
# slack can not be spent later. Rationale and blind spots: docs/architecture/layer-ratchet.md.
#
# Usage:
#   scripts/check_layer_ratchet.sh              # check (CI); exit 1 on drift
#   scripts/check_layer_ratchet.sh --generate   # rewrite the baseline from the tree
#
# LAYER_RATCHET_ROOT=<dir> measures another tree (used by the controls script).
#
set -euo pipefail

ROOT="${LAYER_RATCHET_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
BASELINE="$ROOT/scripts/layer-ratchet-baseline.txt"
KEYS=(symbol_transform_outside_types union_find_outside_net app_library_editor_internals)

cd "$ROOT"

# Every .rs file under crates/ that is not in the excluded prefix $1.
files_outside() {
  find crates -type f -name '*.rs' -not -path '*/target/*' | { grep -v "^$1" || true; } | sort
}

# Hits for one key as `path:line:text`, one per line. The /dev/null operand
# keeps grep printing file names; tr strips CRs so Git Bash agrees with CI.
hits() {
  local prefix pattern
  case "$1" in
    symbol_transform_outside_types)
      prefix='crates/signex-types/'
      pattern='^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?struct[[:space:]]+SymbolTransform\b'
      ;;
    union_find_outside_net)
      # Named helpers, a `find`/`union` taking a parent table, and the
      # path-halving statement every hand-rolled copy contains.
      prefix='crates/signex-net/'
      pattern='fn[[:space:]]+(uf_find|uf_union|find_root)\b|fn[[:space:]]+(find|union)\b[^;{]*parent[[:space:]]*:|parent\[x\][[:space:]]*=[[:space:]]*parent\[parent\[x\]\]'
      ;;
    app_library_editor_internals)
      prefix='crates/signex-app/src/library/'
      pattern='library::editor::[a-z_]+::(state|canvas|updates|views)\b'
      ;;
  esac
  { files_outside "$prefix" | xargs grep -nE "$pattern" /dev/null || true; } | tr -d '\r'
}

count() { hits "$1" | grep -c . || true; }

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# Layer ratchet baseline (scripts/check_layer_ratchet.sh). Counts may only go down."
    echo "# Regenerate with: bash scripts/check_layer_ratchet.sh --generate"
    for k in "${KEYS[@]}"; do echo "$k=$(count "$k")"; done
  } > "$BASELINE.tmp"
  mv "$BASELINE.tmp" "$BASELINE"
  echo "Wrote ${BASELINE#"$ROOT"/}"
  exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
  echo "Layer ratchet: ${BASELINE#"$ROOT"/} is missing; run with --generate."
  exit 1
fi

fail=0
for k in "${KEYS[@]}"; do
  base="$({ grep -E "^$k=" "$BASELINE" || true; } | tail -n 1 | cut -d= -f2 | tr -d '\r ')"
  if ! [[ "$base" =~ ^[0-9]+$ ]]; then
    echo "Layer ratchet: no numeric baseline for '$k' in ${BASELINE#"$ROOT"/}."
    fail=1
    continue
  fi
  now="$(count "$k")"
  if (( now > base )); then
    echo "Layer ratchet: '$k' grew from $base to $now. Hits:"
    hits "$k" | sed 's/^/    /'
    echo "  Put the code in its owning crate / slice instead of adding to the debt."
    fail=1
  elif (( now < base )); then
    echo "Layer ratchet: '$k' fell from $base to $now. Lock in the gain:"
    echo "  bash scripts/check_layer_ratchet.sh --generate, then commit the baseline."
    fail=1
  fi
done

if (( fail )); then
  exit 1
fi
echo "Layer ratchet: OK — counts match scripts/layer-ratchet-baseline.txt."
