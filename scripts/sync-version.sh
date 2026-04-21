#!/usr/bin/env bash
# sync-version.sh — Syncs the workspace version in Cargo.toml with the
# vX.Y semver encoded in the current git branch name.
#
# Branch name patterns recognised:
#   feature/vX.Y-...   →  X.Y.0
#   fix/vX.Y-...       →  X.Y.0
#   hotfix/vX.Y-...    →  X.Y.0
#   vX.Y               →  X.Y.0
#   vX.Y.Z             →  X.Y.Z
#
# If the current major.minor already matches the branch, the existing
# patch number is preserved (so running this twice is idempotent).
#
# Usage:
#   ./scripts/sync-version.sh          # dry-run-safe; exits 0 if nothing to do
#   ./scripts/sync-version.sh --apply  # actually write Cargo.toml

set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
CARGO_TOML="$REPO_ROOT/Cargo.toml"
APPLY=false
[[ "${1:-}" == "--apply" ]] && APPLY=true

# ── Extract version from branch name ───────────────────────────────────────
BRANCH=$(git -C "$REPO_ROOT" branch --show-current)

if [[ "$BRANCH" =~ v([0-9]+)\.([0-9]+)\.([0-9]+) ]]; then
    NEW_MAJOR="${BASH_REMATCH[1]}"
    NEW_MINOR="${BASH_REMATCH[2]}"
    NEW_PATCH="${BASH_REMATCH[3]}"
elif [[ "$BRANCH" =~ v([0-9]+)\.([0-9]+) ]]; then
    NEW_MAJOR="${BASH_REMATCH[1]}"
    NEW_MINOR="${BASH_REMATCH[2]}"
    NEW_PATCH=""          # will be filled in below
else
    echo "ℹ️  Branch '$BRANCH' contains no vX.Y version — nothing to do."
    exit 0
fi

# ── Read current version from Cargo.toml ───────────────────────────────────
CURRENT=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
CUR_MAJOR=$(echo "$CURRENT" | cut -d. -f1)
CUR_MINOR=$(echo "$CURRENT" | cut -d. -f2)
CUR_PATCH=$(echo "$CURRENT" | cut -d. -f3)

# Preserve patch if major.minor is unchanged; reset to 0 otherwise.
if [[ -z "$NEW_PATCH" ]]; then
    if [[ "$CUR_MAJOR" == "$NEW_MAJOR" && "$CUR_MINOR" == "$NEW_MINOR" ]]; then
        NEW_PATCH="$CUR_PATCH"
    else
        NEW_PATCH="0"
    fi
fi

NEW_VERSION="$NEW_MAJOR.$NEW_MINOR.$NEW_PATCH"

if [[ "$CURRENT" == "$NEW_VERSION" ]]; then
    echo "✅ Version already $CURRENT — matches branch '$BRANCH'. Nothing to do."
    exit 0
fi

echo "🔄 Branch : $BRANCH"
echo "   Current: $CURRENT"
echo "   New    : $NEW_VERSION"

if [[ "$APPLY" == "false" ]]; then
    echo ""
    echo "   Dry run — run with --apply to write Cargo.toml."
    exit 0
fi

# ── Apply ───────────────────────────────────────────────────────────────────
# Use perl for portable in-place replacement (sed -i differs on macOS vs GNU).
perl -i -pe "s/^(version = \")$CURRENT(\")$/\${1}$NEW_VERSION\${2}/" "$CARGO_TOML"

# Verify
WRITTEN=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
if [[ "$WRITTEN" == "$NEW_VERSION" ]]; then
    echo "✅ Cargo.toml updated to $NEW_VERSION."
else
    echo "❌ Something went wrong — Cargo.toml shows '$WRITTEN' instead of '$NEW_VERSION'." >&2
    exit 1
fi
