#!/usr/bin/env bash
#
# Core-bound state stays UI-free (ADR-0006, "Enforcement", #2494). The structs
# whose data the slice table moves into the application core (documents,
# drafts, dirty state, histories, owed work, ledgers, check results, the agent
# journal) are named in scripts/uifree-state.txt. Two checks:
#
#   field ratchet  Each field type of a named struct is resolved to an origin.
#                  Allowed: std/core/alloc and primitives, the crates in DOMAIN
#                  of scripts/check_arch_guard.sh (read from that script),
#                  signex-commands, signex-i18n-ids, `crate::app::core`, the
#                  data crates and the UI-free modules the rule file lists.
#                  Every other origin counts: iced, a GUI module of signex-app
#                  (canvas, panels, schematic_runtime, library::editor, ...) and
#                  any unlisted signex-app module. A field counts once, however
#                  many of its type's paths (generic arguments included) are
#                  disallowed. More counted fields than the baseline
#                  (scripts/uifree-state-baseline.txt), or a counted field the
#                  baseline does not name, fails; fewer passes and asks for the
#                  baseline to be lowered.
#   closure        Outside `#[cfg(test)]` items, a listed UI-free module names
#                  only std, the allowed crates, `crate::app::core` and other
#                  listed modules, in its `use` declarations and in the paths
#                  of its code. `super::` / `self::` / `crate::` paths are
#                  resolved first; `#[path]`, `include!` and an `extern crate`
#                  of anything else fail too.
#
# Resolution (scripts/check_uifree_state.awk). A type's paths are resolved
# through the file's module-scope `use` declarations (aliases and `{self}`
# included; a `use` inside a fn or inline `mod` body binds nothing there, and a
# name bound to two different paths is unresolved), explicit `crate::` /
# `self::` / `super::` paths, the module-scope items and `mod`s defined in the
# same file (an item there belongs to that file's module; a `const` / `static`
# never shadows a path's first segment), the std
# prelude and primitives, and signex-app's dependencies. A path whose module is
# listed is allowed; a re-export is judged where it is named, not where it is
# defined. An ident nothing binds (a glob import, a generic parameter of the
# struct, a macro-made name) is UNRESOLVED and counts: the check fails safe.
# Fields come from ast-grep (scripts/uifree-struct-field.yml); `#[cfg(test)]`
# items and fields are skipped (scripts/uifree-test-item.yml); comments and
# strings are dropped and `use` trees flattened by scripts/check_core_guard.awk.
#
# Known blind spots: an inline `mod x { .. }` that is not test code resolves
# `self::` / `super::` against the file's module; a path split over lines in a
# macro body; a macro pulled in by `#[macro_use]`; `cfg(all(test, ..))` code
# counts as production (fail-safe); a type-erased std field (`Box<dyn Any>`,
# `Box<dyn Fn(..)>` capturing GUI state) passes, since only the names in the
# type are judged. The rule file and the baseline are owned by
# the maintainers (CODEOWNERS): adding a module needs Caner's or Hakan's OK.
#
# Usage:
#   scripts/check_uifree_state.sh              # check (CI)
#   scripts/check_uifree_state.sh --generate   # rewrite the baseline
#
# UIFREE_ROOT=<dir> checks another tree and UIFREE_AWK=<awk> picks the awk
# (both used by the controls script).
#
set -euo pipefail

SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
ROOT="${UIFREE_ROOT:-$(cd "$SCRIPTS/.." && pwd)}"
RULES="scripts/uifree-state.txt"
BASELINE="scripts/uifree-state-baseline.txt"
APP_SRC="crates/signex-app/src"
LEXER="$SCRIPTS/check_core_guard.awk"
RESOLVER="$SCRIPTS/check_uifree_state.awk"
FIELD_RULE="$SCRIPTS/uifree-struct-field.yml"
TEST_RULE="$SCRIPTS/uifree-test-item.yml"
AWK="${UIFREE_AWK:-awk}"

cd "$ROOT"
for tool in ast-grep jq; do
  if ! command -v "$tool" >/dev/null; then
    echo "UI-free state: FAIL — $tool is not installed."
    exit 2
  fi
done
for f in "$RULES" scripts/check_arch_guard.sh crates/signex-app/Cargo.toml; do
  if [[ ! -f "$f" ]]; then
    echo "UI-free state: FAIL — no $f under $ROOT."
    exit 2
  fi
done

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Rule-file entries of one kind, `#` comments and blanks dropped.
entries() {
  { grep -E "^$1[[:space:]]" "$RULES" || true; } | sed 's/#.*$//; s/[[:space:]]*$//' | tr -d '\r'
}

# DOMAIN=( ... ) of check_arch_guard.sh, as Rust idents.
domain() {
  awk '/^DOMAIN=\(/ { on = 1; sub(/^DOMAIN=\(/, "") }
       on { line = $0; if (sub(/\).*$/, "", line)) on = 0; print line; if (!on) exit }' \
    scripts/check_arch_guard.sh | tr -s ' \t' '\n' | grep -v '^$' | tr '-' '_'
}

# Every dependency key of signex-app's Cargo.toml, as Rust idents.
deps() {
  awk '/^\[/ { on = ($0 ~ /dependencies\]$/); next }
       on && /^[A-Za-z0-9_-]+[[:space:]]*(=|\.)/ { sub(/[[:space:]]*[=.].*$/, ""); print }' \
    crates/signex-app/Cargo.toml | tr -d '\r' | tr '-' '_'
}

# The module path of a signex-app source file.
modpath() {
  local rel="${1#"$APP_SRC"/}"
  rel="${rel%.rs}"
  rel="${rel%/mod}"
  [[ "$rel" == lib ]] && { echo crate; return; }
  echo "crate::${rel//\//::}"
}

ALLOWED="$(domain | tr '\n' ' ') signex_commands signex_i18n_ids $(entries crate | awk '{ print $2 }' | tr '\n' ' ')"
if ! domain | grep -q .; then
  echo "UI-free state: FAIL — no DOMAIN list found in scripts/check_arch_guard.sh."
  exit 2
fi
DEPS="$(deps | tr '\n' ' ')"
find "$APP_SRC" -type f -name '*.rs' | sort | while IFS= read -r f; do modpath "$f"; done > "$TMP/modules"

status=0
LISTED=""
while read -r _ path; do
  [[ -z "$path" ]] && continue
  if [[ ! -f "$path" || "$path" != "$APP_SRC"/* ]]; then
    echo "UI-free state: FAIL — listed module $path is not a signex-app source file."
    status=1
    continue
  fi
  LISTED="$LISTED $(modpath "$path")"
done < <(entries module)

# feed <file>: the resolver's input records for one file.
feed() {
  local scan
  if ! scan="$(ast-grep scan --rule "$TEST_RULE" --json=compact "$1")"; then
    echo "UI-free state: FAIL — ast-grep could not run $TEST_RULE on $1." >&2
    return 2
  fi
  jq -r '.[] | "T\t\(.range.start.line + 1)\t\(.range.end.line + 1)"' <<<"$scan"
  "$AWK" -v f=L -v depth=0 -v mode=lines -f "$LEXER" "$1"
  [[ "${2:-}" == fields ]] || return 0
  if ! scan="$(ast-grep scan --rule "$FIELD_RULE" --json=compact "$1")"; then
    echo "UI-free state: FAIL — ast-grep could not run $FIELD_RULE on $1." >&2
    return 2
  fi
  jq -r '.[] | .metaVariables.single as $m
         | "F\t\(.range.start.line + 1)\t\($m.STRUCT.text)\t\($m.NAME.text // "")\t\($m.TYPE.text | gsub("\\s+"; " "))"' <<<"$scan"
}

# resolve <file> <mode> [wanted]
resolve() {
  local input
  input="$(feed "$1" "$2")" || return 2
  "$AWK" -v mode="$2" -v file="$1" -v modpath="$(modpath "$1")" -v allowed="$ALLOWED" \
    -v deps="$DEPS" -v listed="$LISTED" -v modules="$TMP/modules" -v wanted="${3:-}" \
    -f "$RESOLVER" <<<"$input"
}

# Field ratchet: one resolver run per file, for the structs listed in it.
entries struct | awk '{ print $2 }' | sort -u > "$TMP/files"
: > "$TMP/counted"
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  if [[ ! -f "$file" ]]; then
    echo "UI-free state: FAIL — $RULES names $file, which does not exist."
    status=1
    continue
  fi
  wanted="$(entries struct | awk -v f="$file" '$2 == f { printf "%s=%s;", $3, ($4 == "" ? "*" : $4) }')"
  out="$(resolve "$file" fields "$wanted")" || exit 2
  { grep '^COUNT' <<<"$out" || true; } >> "$TMP/counted"
  for s in $(entries struct | awk -v f="$file" '$2 == f { print $3 }'); do
    if ! grep -qx "SEEN	$s" <<<"$out"; then
      echo "UI-free state: FAIL — $RULES names struct $s in $file; no such struct (or no matching field) there."
      status=1
    fi
  done
done < "$TMP/files"

count="$(grep -c . "$TMP/counted" || true)"
keys="$(cut -f2 "$TMP/counted" | LC_ALL=C sort)"

if [[ "${1:-}" == "--generate" ]]; then
  {
    echo "# UI-free state baseline (scripts/check_uifree_state.sh, ADR-0006). The count may only go down."
    echo "# Regenerate with: bash scripts/check_uifree_state.sh --generate"
    echo "count=$count"
    awk 'NF { print "field=" $0 }' <<<"$keys"
  } > "$BASELINE.tmp"
  mv "$BASELINE.tmp" "$BASELINE"
  echo "Wrote $BASELINE ($count counted fields)."
  exit 0
fi

# Closure: every listed module, outside its test items.
while read -r _ path; do
  [[ -z "$path" || ! -f "$path" ]] && continue
  out="$(resolve "$path" closure)" || exit 2
  viols="$({ grep '^VIOL' <<<"$out" || true; } | cut -f2-)"
  if [[ -n "$viols" ]]; then
    echo "UI-free state: FAIL — listed module $path is not UI-free:"
    awk '{ print "    " $0 }' <<<"$viols"
    status=1
  fi
done < <(entries module)

if [[ ! -f "$BASELINE" ]]; then
  echo "UI-free state: FAIL — $BASELINE is missing; run with --generate."
  exit 1
fi
base="$({ grep -E '^count=' "$BASELINE" || true; } | tail -n 1 | cut -d= -f2 | tr -d '\r ')"
if ! [[ "$base" =~ ^[0-9]+$ ]]; then
  echo "UI-free state: FAIL — no numeric count= line in $BASELINE."
  exit 1
fi
known="$({ grep -E '^field=' "$BASELINE" || true; } | cut -d= -f2- | tr -d '\r' | LC_ALL=C sort)"
new_keys="$(LC_ALL=C comm -23 <(grep -v '^$' <<<"$keys" || true) <(grep -v '^$' <<<"$known" || true))"

if (( count > base )) || [[ -n "$new_keys" ]]; then
  echo "UI-free state: FAIL — $count counted fields (baseline $base). New:"
  while IFS= read -r k; do
    [[ -z "$k" ]] && continue
    awk -F'\t' -v k="$k" '$2 == k { print "    " $3 }' "$TMP/counted"
  done <<<"$new_keys"
  (( count > base )) && [[ -z "$new_keys" ]] && cut -f3 "$TMP/counted" | sed 's/^/    /'
  echo "  Give the field a type from std, a DOMAIN crate or app/core/, or move its type to a"
  echo "  module listed as UI-free in $RULES first (ADR-0006, Enforcement)."
  echo "  A renamed or moved counted field needs --generate and a maintainer's approval of $BASELINE."
  status=1
elif (( count < base )); then
  echo "UI-free state: $count counted fields, below the baseline of $base. Lower it:"
  echo "  bash scripts/check_uifree_state.sh --generate, then commit $BASELINE."
fi

if (( status )); then
  exit 1
fi
echo "UI-free state: OK — $count counted fields (baseline $base), $(entries module | grep -c . || true) UI-free modules."
