#!/usr/bin/env bash
#
# Lint-lockstep gate — a workspace lint must reach every member, including the
# ones that cannot say `[lints] workspace = true`.
#
# Most members inherit with `[lints] workspace = true`. `signex-app` cannot:
# cargo rejects mixing inheritance with per-lint overrides —
#
#   cannot override `workspace.lints` in `lints`, either remove the overrides
#   or `lints.workspace = true` and manually specify the lints
#
# — and that crate needs `[lints.clippy] let_underscore_must_use = "warn"`,
# which the workspace deliberately does not carry (lifting it fires 33 times
# across signex-types / signex-output / signex-library / signex-gfx, none of
# them `Task` discards). So it re-declares the workspace lints by hand.
#
# Nothing enforced that by-hand copy (GH #570). A lint added to
# `[workspace.lints.rust]` silently skipped the largest crate in the repo —
# no error, no warning, no test. This script is that missing check.
#
# The rule is containment, not equality:
#
#   for every non-inheriting member, every `[workspace.lints.<group>]` key
#   must appear in that member's `[lints.<group>]` at the SAME level.
#
# Extra member-local lints are fine — that divergence is the whole reason the
# crate opted out. A missing key is a hole; a weaker level is a silent
# downgrade. Both fail.
#
# Deliberately generic rather than hardcoding `signex-app`: the next crate
# that needs its own override gets the same protection without a script edit.
#
# Usage:
#   scripts/check_lint_lockstep.sh    # check (CI); exit 1 on a violation
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Emit `<group> <key> <level>` for every lint under a TOML table whose header
# matches `want` (e.g. `workspace.lints` or `lints`). Handles both spellings
# cargo accepts:
#
#   unused_must_use = "deny"
#   unused_must_use = { level = "deny", priority = -1 }
#
# Comments and blank lines are skipped; a table header ends the previous one.
lint_rows() {
  local file="$1" want="$2"
  awk -v want="$want" '
    { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
    /^#/ || /^$/ { next }
    /^\[/ {
      hdr = $0
      sub(/^\[[[:space:]]*/, "", hdr); sub(/[[:space:]]*\].*$/, "", hdr)
      group = ""
      if (index(hdr, want ".") == 1) group = substr(hdr, length(want) + 2)
      next
    }
    group == "" { next }
    # key = "level"   |   key = { level = "level", ... }
    {
      line = $0
      eq = index(line, "=")
      if (eq == 0) next
      key = substr(line, 1, eq - 1)
      val = substr(line, eq + 1)
      sub(/[[:space:]]+$/, "", key); sub(/^[[:space:]]+/, "", val)
      if (val ~ /^\{/) {
        if (match(val, /level[[:space:]]*=[[:space:]]*"[^"]*"/) == 0) next
        lvl = substr(val, RSTART, RLENGTH)
        sub(/^level[[:space:]]*=[[:space:]]*"/, "", lvl); sub(/"$/, "", lvl)
      } else {
        if (match(val, /^"[^"]*"/) == 0) next
        lvl = substr(val, RSTART, RLENGTH); gsub(/"/, "", lvl)
      }
      print group, key, lvl
    }
  ' "$file"
}

# True when a member inherits wholesale — `[lints] workspace = true`.
inherits_workspace() {
  awk '
    { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
    /^#/ || /^$/ { next }
    /^\[/ { in_lints = ($0 ~ /^\[[[:space:]]*lints[[:space:]]*\]/); next }
    in_lints && /^workspace[[:space:]]*=[[:space:]]*true[[:space:]]*$/ { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$1"
}

WS_ROWS="$(lint_rows Cargo.toml workspace.lints)"

if [[ -z "$WS_ROWS" ]]; then
  echo "OK  root Cargo.toml declares no [workspace.lints.*] — nothing to keep in lockstep."
  exit 0
fi

fail=0
checked=0

while IFS= read -r manifest; do
  [[ -f "$manifest" ]] || continue
  # A member with no [lints] table at all inherits nothing and overrides
  # nothing. That is its own hazard, but it is not the lockstep hole this
  # gate guards, and every current member has one. Skip quietly.
  grep -qE '^[[:space:]]*\[[[:space:]]*lints' "$manifest" || continue
  inherits_workspace "$manifest" && continue

  checked=$((checked + 1))
  member_rows="$(lint_rows "$manifest" lints)"

  while read -r group key level; do
    [[ -n "$group" ]] || continue
    got="$(awk -v g="$group" -v k="$key" '$1 == g && $2 == k { print $3; exit }' <<<"$member_rows")"
    if [[ -z "$got" ]]; then
      echo "FAIL  $manifest — missing [lints.$group] $key (root has it at \"$level\")."
      echo "      This crate does not inherit \`[lints] workspace = true\`, so a lint"
      echo "      added to [workspace.lints.$group] never reaches it. Copy the line in,"
      echo "      or make the crate inherit (GH #570)."
      fail=1
    elif [[ "$got" != "$level" ]]; then
      echo "FAIL  $manifest — [lints.$group] $key is \"$got\" but the workspace says \"$level\"."
      echo "      A hand-copied lint that drifts to a weaker level is a silent"
      echo "      downgrade in the one crate the lint matters most (GH #570)."
      fail=1
    fi
  done <<<"$WS_ROWS"
done < <(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml -not -path '*/target/*' | sort)

if (( fail )); then
  echo
  echo "The by-hand copy in a non-inheriting member has drifted from"
  echo "[workspace.lints.*] in the root Cargo.toml. Extra member-local lints are"
  echo "fine — that is why the crate opted out — but every workspace lint must be"
  echo "present at the same level."
  exit 1
fi

echo "OK  lint lockstep holds ($checked non-inheriting member(s) checked against [workspace.lints.*])."
