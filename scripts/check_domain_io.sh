#!/usr/bin/env bash
#
# Domain IO gate (#2431, #2326 principle S3): the DOMAIN crates listed in
# scripts/check_arch_guard.sh do no IO and read no clock or randomness, so the
# same core runs on the desktop, in a server and in a browser. Clippy's
# `disallowed-methods` with the ban list in scripts/domain-clippy/clippy.toml
# (std::fs, Path probes, Instant/SystemTime/chrono ::now, Uuid::now_v7/new_v4)
# runs over their lib and bin targets only, so tests are exempt.
#
# Sites not moved yet are listed in scripts/domain-io-allowlist.txt as
# `<file> <hits>  # reason`. The gate fails on a hit in a file not listed, and
# on a listed count that differs from the tree (higher: new IO; lower: lower
# the number), so the list only shrinks. Uuid hits in signex-engine belong to
# scripts/check_engine_ids.sh (#2429) and are not counted here.
# Blind spot: the count is per file, so swapping one banned call for another in
# an allowlisted file passes; review catches that.
#
# Usage: scripts/check_domain_io.sh
# DOMAIN_IO_CLIPPY_JSON=<file> reads clippy's JSON from a file and
# DOMAIN_IO_ALLOWLIST=<file> overrides the list (scripts/test_check_domain_io.sh).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ALLOWLIST="${DOMAIN_IO_ALLOWLIST:-$ROOT/scripts/domain-io-allowlist.txt}"
cd "$ROOT"

# The one DOMAIN list lives in check_arch_guard.sh.
domain_crates() {
  awk '/^DOMAIN=\(/ { on = 1; next } on && /^\)/ { exit } on' scripts/check_arch_guard.sh \
    | tr -s '[:space:]' '\n' | grep -v '^$'
}

clippy_json() {
  if [[ -n "${DOMAIN_IO_CLIPPY_JSON:-}" ]]; then
    cat "$DOMAIN_IO_CLIPPY_JSON"
    return
  fi
  local args=() crate
  while IFS= read -r crate; do args+=(-p "$crate"); done < <(domain_crates)
  if [[ ${#args[@]} -eq 0 ]]; then
    echo "Domain IO gate: FAIL — no DOMAIN list in scripts/check_arch_guard.sh." >&2
    exit 2
  fi
  CLIPPY_CONF_DIR="$ROOT/scripts/domain-clippy" \
    cargo clippy --locked --all-features --message-format=json "${args[@]}"
}

# A build error must fail the gate, never read as "no hits".
if ! json="$(clippy_json)"; then
  echo "Domain IO gate: FAIL — clippy did not build the DOMAIN crates."
  exit 2
fi

# `<file> <hits>`, one row per file, each site counted once.
found="$(
  jq -r '
    select(.reason == "compiler-message") | .message
    | select(.code.code == "clippy::disallowed_methods")
    | (.spans[] | select(.is_primary)) as $s
    | [$s.file_name, $s.line_start, $s.column_start, .message] | @tsv
  ' <<<"$json" | tr -d '\r' | sort -u \
    | awk -F'\t' '!($1 ~ /^crates\/signex-engine\// && $4 ~ /uuid::Uuid::/) { n[$1]++ }
        END { for (f in n) print f, n[f] }' | sort
)"
listed=""
if [[ -f "$ALLOWLIST" ]]; then
  listed="$(
    { grep -vE '^[[:space:]]*(#|$)' "$ALLOWLIST" || true; } \
      | awk '{ print $1, ($2 ~ /^[0-9]+$/ ? $2 : 0) }' | sort
  )"
fi

report="$(
  awk '
    FNR == NR { if (NF) listed[$1] = $2; next }
    NF { found[$1] = $2 }
    END {
      for (f in found) {
        if (!(f in listed)) print "new", f, found[f]
        else if (found[f] > listed[f]) print "grown", f, listed[f], found[f]
        else if (found[f] < listed[f]) print "shrunk", f, listed[f], found[f]
      }
      for (f in listed) if (!(f in found)) print "stale", f
    }
  ' <(printf '%s\n' "$listed") <(printf '%s\n' "$found") | sort
)"

status=0
section() {
  local kind="$1" title="$2" rows
  rows="$(printf '%s\n' "$report" | awk -v k="$kind" '$1 == k { $1 = ""; sub(/^ /, ""); print "  " $0 }')"
  [[ -z "$rows" ]] && return 0
  echo "Domain IO gate: FAIL — $title"
  printf '%s\n' "$rows"
  status=1
}
section new "IO, clock or id minting in a DOMAIN crate; take it from the host (file, hits):"
section grown "more IO in an allowlisted file; take it from the host (file, listed, found):"
section shrunk "fewer hits than listed; lower the count in ${ALLOWLIST#"$ROOT"/} (file, listed, found):"
section stale "no hits left; delete the entry from ${ALLOWLIST#"$ROOT"/}:"
if [[ $status -eq 0 ]]; then
  echo "Domain IO gate: OK — DOMAIN crates do IO only in the $(printf '%s\n' "$listed" | grep -c . || true) allowlisted files."
fi
exit $status
