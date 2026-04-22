#!/usr/bin/env bash
# PreToolUse hook — fires before every Bash call in this repo.
#
# When a release-tag push is about to happen, this guard verifies that
# CHANGELOG.md already has an entry for the tag about to ship. If the
# entry is missing, it blocks the push with exit 2 and tells the
# current Claude Code session to sync docs first.
#
# Pairs with on-release-push.sh (PostToolUse), which handles *website*
# sync after a successful push — a sync that doesn't need to be in the
# tagged commit, just needs to fire reliably once the tag is live.
#
# The entire flow runs inside whatever Claude Code subscription the
# contributor is signed into — no API key, no CI billing.

set -euo pipefail

if ! input="$(cat)"; then
  exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
  # Silent no-op if jq is missing — never break a session.
  exit 0
fi

tool_name=$(echo "$input" | jq -r '.tool_name // empty')
[[ "$tool_name" != "Bash" ]] && exit 0

command=$(echo "$input" | jq -r '.tool_input.command // empty')
[[ -z "$command" ]] && exit 0

# Only care about git-push commands that ship a semver tag.
if ! echo "$command" | grep -qE 'git[[:space:]]+push[[:space:]].*v[0-9]+\.[0-9]+\.[0-9]+'; then
  # Also catch `git push --tags` which pushes every local tag.
  if ! echo "$command" | grep -qE 'git[[:space:]]+push[[:space:]].*--tags'; then
    exit 0
  fi
fi

tag=""
if echo "$command" | grep -qE 'v[0-9]+\.[0-9]+\.[0-9]+'; then
  tag=$(echo "$command" | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' | head -1)
else
  repo_dir="${CLAUDE_PROJECT_DIR:-$PWD}"
  # Picking the newest local tag that isn't already on the remote — that's
  # the one about to ship. Falls back to the newest tag of any kind.
  tag=$(git -C "$repo_dir" tag --list 'v*.*.*' --sort=-creatordate 2>/dev/null | head -1 || true)
fi
[[ -z "$tag" ]] && exit 0

version="${tag#v}"
repo_dir="${CLAUDE_PROJECT_DIR:-$PWD}"
changelog="$repo_dir/CHANGELOG.md"
claude_md="$repo_dir/.claude/CLAUDE.md"
readme="$repo_dir/README.md"
internal_plan="$repo_dir/docs/internal/docs/MASTER_PLAN.md"

if [[ ! -f "$changelog" ]]; then
  cat >&2 <<EOF
{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"Refusing to push tag ${tag} — CHANGELOG.md is missing. Create it with an entry for ${tag} before tagging so the release workflow uses the rich notes instead of the 5-bullet auto-generator."}}
EOF
  exit 0
fi

# Collect every docs-first check that failed so the reminder tells the
# session exactly what's still stale, instead of surfacing them one at a
# time across repeated push attempts.
missing=()

if ! grep -qE "^##\s+\[?${version}\]?" "$changelog"; then
  missing+=("CHANGELOG.md — no \"## [${version}]\" section")
fi

if [[ -f "$readme" ]] && ! grep -qF "v${version}" "$readme"; then
  missing+=("README.md — version badge / status table still references the previous tag")
fi

if [[ -f "$claude_md" ]] && ! grep -qE "v${version}.*✅" "$claude_md"; then
  missing+=(".claude/CLAUDE.md — v${version} is not flipped to ✅ in the Versioning section")
fi

# Submodule progress markers are the step that was silently skipped on
# v0.7.0: check MASTER_PLAN.md actually references the current version
# and doesn't still mark it as 🔄 / "in flight".
if [[ -f "$internal_plan" ]]; then
  if ! grep -qF "v${version}" "$internal_plan"; then
    missing+=("docs/internal/docs/MASTER_PLAN.md — no mention of v${version}; progress markers not updated")
  elif grep -qE "v${version}.*🔄|v${version}.*in[ -]flight|v${version}.*in progress" "$internal_plan"; then
    missing+=("docs/internal/docs/MASTER_PLAN.md — v${version} still marked 🔄 / in flight")
  fi
fi

# If the submodule has any uncommitted work, the pointer bump hasn't
# happened yet — that means the outer commit is about to tag a stale
# submodule SHA.
if [[ -d "$repo_dir/docs/internal/.git" ]] || [[ -f "$repo_dir/docs/internal/.git" ]]; then
  if ! git -C "$repo_dir/docs/internal" diff --quiet 2>/dev/null || \
     ! git -C "$repo_dir/docs/internal" diff --cached --quiet 2>/dev/null; then
    missing+=("docs/internal submodule — uncommitted changes; commit + push inside submodule first, then back-bump the pointer in the outer repo")
  fi
fi

if (( ${#missing[@]} > 0 )); then
  reasons=""
  for item in "${missing[@]}"; do
    reasons+="  - ${item}"$'\n'
  done
  cat <<EOF
<system-reminder>
Blocked pushing tag ${tag} — docs-first checklist is incomplete:

${reasons}
Release binaries are built from the tagged commit — if CHANGELOG.md and
README / ROADMAP / CLAUDE.md don't already reflect the new version at
tag time, source downloads from the GitHub Release carry stale docs and
the Release body falls back to GitHub's auto-generated PR-title summary
(usually 3–5 bullets).

Before re-running the push, do all of this in order, in this same session:

1. **CHANGELOG.md** — add a "## [${version}] — <today's date>" section with
   the full feature-level changelog. Model the formatting on the existing
   v0.7.0 entry. Include headings for ERC / Editor / Multi-window / Brand
   / Refactors etc. as appropriate to what shipped.
2. **README.md** — bump the version badge at the top; flip any "What's
   next" row for ${version} from in-progress to shipped.
3. **docs/ROADMAP.md** and **docs/REPOSITORY_AND_CODEBASE.md** — flip
   ${version}'s status marker (🔄 → ✅) and update "current version" fields.
4. **.claude/CLAUDE.md** — flip ${version} in the Versioning section from 🔄 to ✅.
5. **docs/internal submodule** — inside the submodule, update
   MASTER_PLAN.md progress markers, ARCHITECTURE.md if architectural
   surface changed, and any other progress-tracking files under
   docs/internal/docs/. Commit inside the submodule on a matching
   branch, push it, then bump the submodule pointer in the outer repo.
6. **Commit all of the above** on a feature branch (chore/release-prep-${tag}),
   push it, open a PR against dev, get it merged into dev, then cascade
   dev → main via a normal release PR — and THEN tag and push.

Do the work now. When every item above is fixed, the re-run of this
push will succeed.
</system-reminder>
EOF
  # exit 2 tells Claude Code to treat the stderr/stdout as a feedback
  # signal and abort the tool call. The reminder is surfaced to the
  # model automatically so it can drive the sync.
  exit 2
fi

exit 0
