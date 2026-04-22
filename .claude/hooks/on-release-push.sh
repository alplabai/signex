#!/usr/bin/env bash
# PostToolUse hook — fires after every Bash tool call in this repo.
#
# When a release-tag push is detected, emits a system-reminder so the
# current Claude Code session immediately syncs:
#   (A) signex's own public docs (README, ROADMAP, CHANGELOG)
#   (B) the signex.dev marketing site (separate repo)
#
# Runs for everyone who clones signex + opens it in Claude Code — uses
# whatever Claude Code subscription the contributor is signed into.
# No API key, no CI billing.

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

# Detect a push that includes a semver tag.
tag=""
if echo "$command" | grep -qE 'git[[:space:]]+push[[:space:]].*v[0-9]+\.[0-9]+\.[0-9]+'; then
  tag=$(echo "$command" | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' | head -1)
elif echo "$command" | grep -qE 'git[[:space:]]+push[[:space:]].*--tags'; then
  repo_dir="${CLAUDE_PROJECT_DIR:-$PWD}"
  tag=$(git -C "$repo_dir" tag --list 'v*.*.*' --sort=-creatordate 2>/dev/null | head -1 || true)
fi
[[ -z "$tag" ]] && exit 0

success=$(echo "$input" | jq -r '.tool_response.success // true')
[[ "$success" != "true" ]] && exit 0

cat <<EOF
<system-reminder>
Release tag ${tag} was just pushed to signex.

Please sync both the public docs and the marketing site now, in this same
session. Don't wait for a second ask — just do it.

Part A — signex docs, public + internal (this repo + docs/internal submodule):
  1. Read the release notes (gh release view ${tag} or the matching CHANGELOG entry).
  2. Update README.md:
       * Version badge at the top.
       * "What's next" table, if a roadmap row needs to flip from in-progress to shipped.
  3. Update docs/ROADMAP.md and docs/REPOSITORY_AND_CODEBASE.md if either
     references the old version number or status.
  4. If CHANGELOG.md exists and does not already contain an entry for ${tag},
     generate one from git log since the previous tag.
  5. Update the private docs/internal/ submodule. Inside the submodule:
       * MASTER_PLAN.md — flip completed-milestone checkboxes, bump
         "current version" or similar status fields.
       * ARCHITECTURE.md — only if the release shipped an architectural
         change (e.g. multi-window, new crate, storage format).
       * Any other progress-tracking files under docs/internal/docs/.
     Commit inside the submodule on a matching branch, push, then
     back-bump the submodule pointer in the outer repo.
  6. Commit the outer repo's README/ROADMAP/CHANGELOG changes + the
     submodule pointer bump together on a feature branch
     (chore/docs-sync-${tag}) and open a PR against dev. NEVER push
     directly to main or dev.

Part B — signex.dev website (separate repo at ../signex-website):
  1. cd to the signex-website repo (typically ../signex-website or
     ~/Documents/GitHub/signex-website).
  2. Edit index.html:
       * Hero eyebrow: "vX.Y in development — open source, Apache-2.0".
       * JSON-LD softwareVersion.
       * Roadmap <ol class="timeline"> — previous tl-now → tl-done,
         next-in-progress → tl-now, and refresh descriptions if the
         release notes named new shipped features.
       * "What's in the box" feature cards — only edit if a release
         materially changed what ships; never duplicate a card.
  3. Preserve HTML structure, class names, whitespace style, and tone.
  4. Do not touch nav, footer, SVG assets, Organization JSON-LD, styles.css,
     or scripts.js.
  5. Commit on main of signex-website with message "chore(website): sync
     to ${tag}" and push. The deploy.yml workflow rsyncs it to Hostinger.

If the user has explicitly deferred either part, skip that part and
acknowledge. Otherwise proactively do both.
</system-reminder>
EOF
