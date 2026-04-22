#!/usr/bin/env bash
# PostToolUse hook — fires after every Bash call in this repo.
#
# When a release-tag push succeeds, emits a system-reminder telling the
# current Claude Code session to sync the signex.dev marketing site.
# Docs inside signex itself are expected to be correct already —
# pre-release-guard.sh (PreToolUse) blocks any tag push that doesn't
# have a CHANGELOG.md entry for that version, so by the time we get
# here the public-facing repo is already in a consistent state.
#
# The website lives in a separate repo (../signex-website) and is not
# part of the release commit, so it genuinely needs post-tag syncing.
#
# Runs entirely inside whatever Claude Code subscription the
# contributor is signed into — no API key, no CI billing.

set -euo pipefail

if ! input="$(cat)"; then
  exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

tool_name=$(echo "$input" | jq -r '.tool_name // empty')
[[ "$tool_name" != "Bash" ]] && exit 0

command=$(echo "$input" | jq -r '.tool_input.command // empty')
[[ -z "$command" ]] && exit 0

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
Release tag ${tag} was just pushed. The signex repo itself is already
in sync (pre-release-guard.sh verified CHANGELOG.md and friends before
the push), so the only remaining step is the signex.dev marketing site.

Sync ../signex-website now, in this same session:

1. cd to the signex-website repo (typically ../signex-website or
   ~/Documents/GitHub/signex-website).
2. Edit index.html:
     * Hero eyebrow: "v<next-in-dev> in development — open source, Apache-2.0".
     * JSON-LD softwareVersion → ${tag}.
     * Roadmap <ol class="timeline"> — previous tl-now → tl-done,
       next-in-progress → tl-now, refresh descriptions if new features shipped.
     * "What's in the box" feature cards — only edit if this release
       materially changed what ships; never duplicate a card.
3. Preserve HTML structure, class names, whitespace style, and tone.
4. Do not touch nav, footer, SVG assets, Organization JSON-LD, styles.css,
   or scripts.js.
5. Commit on main of signex-website with message
   "chore(website): sync to ${tag}" and push. The deploy.yml workflow
   rsyncs the change to Hostinger automatically.

If the user has explicitly deferred the website update, acknowledge and
skip. Otherwise proactively do it.
</system-reminder>
EOF
