#!/usr/bin/env bash
# UserPromptSubmit hook — fires once per user message.
#
# Matches release intents ("release v0.7.0", "cut v0.8.0", etc.) and
# emits a checklist reminder BEFORE the session starts doing git work,
# so Claude does the doc sync first instead of finding out only when
# the pre-release-guard blocks the tag push.

set -euo pipefail

if ! input="$(cat)"; then
  exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

prompt=$(echo "$input" | jq -r '.prompt // empty')
[[ -z "$prompt" ]] && exit 0

# Match release intent:
#   "release v0.7.0", "release 0.7.0", "cut v0.7.0", "ship v0.7.0",
#   "tag v0.7.0", "publish v0.7.0"
if ! echo "$prompt" | grep -qiE '(release|cut|ship|tag|publish)[[:space:]]+v?[0-9]+\.[0-9]+\.[0-9]+'; then
  exit 0
fi

tag=$(echo "$prompt" | grep -oiE 'v?[0-9]+\.[0-9]+\.[0-9]+' | head -1)
# Normalise to leading-v form.
if [[ "$tag" != v* ]]; then
  tag="v$tag"
fi
version="${tag#v}"

cat <<EOF
<system-reminder>
Release intent detected for ${tag}. Before touching git, run the full
pre-release doc sync in THIS session. The pre-release-guard hook will
block the tag push if these aren't done first:

1. **CHANGELOG.md** — add a "## [${version}] — <today>" section with
   feature-level detail. Use the existing v0.7.0 entry as a template.
2. **README.md** — bump the version badge; flip any ${version} row in
   "What's next" from in-progress to shipped.
3. **docs/ROADMAP.md** + **docs/REPOSITORY_AND_CODEBASE.md** — flip the
   status marker for ${version} (🔄 → ✅), update "current version".
4. **.claude/CLAUDE.md** — flip ${version} in the Versioning section.
5. **docs/internal** submodule — MASTER_PLAN.md progress markers,
   ARCHITECTURE.md if the release shipped an architectural change,
   other docs/internal/docs/ progress files. Commit inside the
   submodule on chore/docs-sync-${tag}, push it, back-bump the
   submodule pointer in the outer repo.
6. **Commit + PR**: feature branch chore/release-prep-${tag} → dev (PR),
   then dev → main (PR), then tag ${tag} on main, then push.

Do the website sync (../signex-website, separate repo) as the LAST step —
handled by the PostToolUse on-release-push.sh hook after the tag push
succeeds. You don't need to drive that part manually.
</system-reminder>
EOF
