#!/usr/bin/env bash
# Fail when a Linux release binary needs a newer glibc than the oldest
# distribution Signex supports (#1688). Usage: check_glibc_ceiling.sh <binary>
set -euo pipefail

CEILING="${GLIBC_CEILING:-2.35}"
bin="${1:?usage: check_glibc_ceiling.sh <binary>}"

# GLIBCXX_/CXXABI_ are not checked: the 22.04 libstdc++ is older than any
# supported distribution's. Every versioned glibc symbol the binary imports, newest last.
newest="$(objdump -T "$bin" | { grep -o 'GLIBC_[0-9][0-9.]*' || true; } | sed 's/GLIBC_//' | sort -Vu | tail -n 1)"
if [[ -z "$newest" ]]; then
  echo "::error::no GLIBC symbol versions found in $bin"
  exit 1
fi

highest="$(printf '%s\n%s\n' "$newest" "$CEILING" | sort -V | tail -n 1)"
if [[ "$highest" != "$CEILING" ]]; then
  echo "::error::$bin needs GLIBC_$newest; the ceiling is GLIBC_$CEILING"
  exit 1
fi
echo "glibc ceiling OK: $bin needs GLIBC_$newest (ceiling GLIBC_$CEILING)"
