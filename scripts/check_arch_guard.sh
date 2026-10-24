#!/usr/bin/env bash
#
# Architecture guard (ADR-0001 D5: "a boundary that is not machine-checked
# erodes"). Two rules, both read from `cargo metadata`:
#
#   1. Workspace edges are an allow-list. Every normal (non-dev, non-build)
#      dependency from one workspace crate on another must appear in
#      scripts/arch-edges.txt. Adding an edge is allowed — but it is a
#      deliberate, reviewed line in that file, not a side effect of a
#      Cargo.toml edit. Removing an edge from the code without removing its
#      line also fails, so the table stays the true graph.
#   2. Domain crates stay UI-free. The crates listed under DOMAIN below may not
#      depend on the UI / GPU stack (iced*, wgpu, winit, cryoglyph, rfd) or on
#      signex-widgets / signex-app, directly.
#
# Dev-dependencies are exempt: a test may pull in anything.
#
# Usage:
#   scripts/check_arch_guard.sh            # check (CI); exit 1 on a violation
#   scripts/check_arch_guard.sh --generate # rewrite arch-edges.txt from the tree
#
# ARCH_GUARD_METADATA=<file> reads metadata JSON from a file instead of running
# cargo (used by scripts/test_check_arch_guard.sh).
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EDGES="${ARCH_GUARD_EDGES:-$ROOT/scripts/arch-edges.txt}"

# Crates holding domain logic: no UI toolkit, no GPU, no app.
DOMAIN=(
  signex-types signex-net signex-erc signex-erc-dsl signex-engine
  signex-bom signex-output signex-library signex-sketch signex-bake
  signex-3d-model-importer
)
# Matched as exact names or, with a trailing *, as a prefix.
UI_STACK=(iced 'iced_*' wgpu 'wgpu-*' naga winit cryoglyph glyphon rfd signex-widgets signex-app)

metadata() {
  if [[ -n "${ARCH_GUARD_METADATA:-}" ]]; then
    cat "$ARCH_GUARD_METADATA"
  else
    (cd "$ROOT" && cargo metadata --format-version 1 --no-deps)
  fi
}

META="$(metadata)"

# "from -> to" for every normal intra-workspace dependency, sorted.
current_edges() {
  jq -r '
    [.packages[].name] as $ws
    | .packages[]
    | .name as $from
    | .dependencies[]
    | select(.kind == null)
    | select(.name as $n | $ws | index($n))
    | "\($from) -> \(.name)"
  ' <<<"$META" | sort -u
}

# The table's edges, one per line, without comments or blanks. `grep -v`
# exits 1 when every line is a comment, which `set -e` / pipefail would turn
# into a silent abort, so an all-comment table must read as empty.
table_edges() {
  [[ -f "$EDGES" ]] || return 0
  { grep -v '^[[:space:]]*#' "$EDGES" || true; } \
    | sed 's/[[:space:]]*#.*$//; s/[[:space:]]*$//; /^[[:space:]]*$/d' | sort -u
}

if [[ "${1:-}" == "--generate" ]]; then
  # Keep the trailing `# ...` note of every edge that is still present, so a
  # regenerate does not drop recorded debt (e.g. `# known debt (#664)`).
  # Bash 3.2 (stock macOS) has no associative arrays, so the notes go
  # through a tab-separated temp file and awk does the lookup.
  notes_file="$(mktemp)"
  trap 'rm -f "$notes_file"' EXIT
  if [[ -f "$EDGES" ]]; then
    awk '!/^[[:space:]]*#/ && /#/ {
      i = index($0, "#"); edge = substr($0, 1, i - 1); sub(/[[:space:]]+$/, "", edge)
      print edge "\t  #" substr($0, i + 1)
    }' "$EDGES" > "$notes_file"
  fi
  generated="$(current_edges)"
  {
    echo "# Allowed workspace dependency edges (normal deps only)."
    echo "# Checked by scripts/check_arch_guard.sh. Add a line to add an edge."
    awk -F'\t' 'FILENAME == ARGV[1] { note[$1] = $2; next } $0 != "" { print $0 note[$0] }' \
      "$notes_file" - <<<"$generated"
  } > "$EDGES.tmp"
  mv "$EDGES.tmp" "$EDGES"
  echo "Wrote $(table_edges | grep -c .) edges to ${EDGES#"$ROOT"/}"
  exit 0
fi

fail=0

allowed="$(table_edges)"
actual="$(current_edges)"

while IFS= read -r edge; do
  [[ -z "$edge" ]] && continue
  echo "Arch guard: new workspace edge '$edge' is not in scripts/arch-edges.txt."
  echo "  If it is intended, add that line in the same PR so the review sees it."
  fail=1
done < <(comm -13 <(echo "$allowed") <(echo "$actual"))

while IFS= read -r edge; do
  [[ -z "$edge" ]] && continue
  echo "Arch guard: scripts/arch-edges.txt lists '$edge', which no Cargo.toml has any more."
  echo "  Delete the line so the table stays the real graph."
  fail=1
done < <(comm -23 <(echo "$allowed") <(echo "$actual"))

for crate in "${DOMAIN[@]}"; do
  deps="$(jq -r --arg c "$crate" '
    .packages[] | select(.name == $c) | .dependencies[]
    | select(.kind == null) | .name
  ' <<<"$META")"
  while IFS= read -r dep; do
    [[ -z "$dep" ]] && continue
    for banned in "${UI_STACK[@]}"; do
      # shellcheck disable=SC2053 # glob match on purpose
      if [[ "$dep" == $banned ]]; then
        echo "Arch guard: domain crate '$crate' depends on '$dep' (UI/GPU stack)."
        echo "  Domain crates stay UI-free (ADR-0001); move the code to an app-side crate."
        fail=1
      fi
    done
  done <<<"$deps"
done

if [[ $fail -eq 0 ]]; then
  echo "Arch guard: OK — $(echo "$actual" | grep -c .) workspace edges match the table; domain crates are UI-free."
fi
exit $fail
