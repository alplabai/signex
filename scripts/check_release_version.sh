#!/usr/bin/env bash
#
# Release version-consistency gate (GH #343) — the tag, the workspace version
# and the CHANGELOG section must name the same release.
#
#   scripts/check_release_version.sh <tag|ref> [repo-root]
#
# <tag> is `vX.Y.Z` or `vX.Y.Z-rc.1` (a `refs/tags/` prefix is tolerated, so
# CI can pass "$GITHUB_REF"). It fails unless all of these hold:
#
#   1. root Cargo.toml `[workspace.package] version` == the tag without its `v`
#   2. CHANGELOG.md has a `## [X.Y.Z]` (or `## X.Y.Z`) heading for that version
#   3. that heading carries a YYYY-MM-DD date and is not marked `unreleased`
#
# On failure the message prints all three values side by side. The heading
# match mirrors the extractor in release.yml ("Extract CHANGELOG entry for this
# tag"): `## [VER]` or `## VER`, leading `v` stripped. The extractor stays
# forgiving (it falls back to auto-generated notes); this script is the strict
# gate in front of it.

set -u

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 <tag> [repo-root]" >&2
  exit 2
fi

TAG="${1#refs/tags/}"
ROOT="${2:-.}"
CARGO="$ROOT/Cargo.toml"
CHANGELOG="$ROOT/CHANGELOG.md"

# Strict semver tag, same shape the release workflow triggers on.
if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-.+)?$ ]]; then
  echo "error: '$TAG' is not a release tag (expected vX.Y.Z or vX.Y.Z-pre)" >&2
  exit 2
fi
VER="${TAG#v}"

for f in "$CARGO" "$CHANGELOG"; do
  if [[ ! -f "$f" ]]; then
    echo "error: $f not found" >&2
    exit 2
  fi
done

# version = "..." inside [workspace.package], and nowhere else.
CARGO_VER="$(awk '
  /^\[/ { in_pkg = ($0 ~ /^\[workspace\.package\][[:space:]]*$/); next }
  in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
    v = $0
    sub(/^[^"]*"/, "", v)
    sub(/".*$/, "", v)
    print v
    exit
  }
' "$CARGO")"

# First heading naming VER: `## [VER]...` or `## VER...`, where VER must end at
# `]`, whitespace or end of line (so 0.1.0 does not match 0.1.01).
HEADING="$(awk -v ver="$VER" '
  /^##[[:space:]]+/ {
    s = $0
    sub(/^##[[:space:]]+/, "", s)
    sub(/^\[/, "", s)
    if (substr(s, 1, length(ver)) == ver) {
      next_ch = substr(s, length(ver) + 1, 1)
      if (next_ch == "" || next_ch == "]" || next_ch ~ /[[:space:]]/) {
        print $0
        exit
      }
    }
  }
' "$CHANGELOG" | tr -d '\r')"

problems=()

if [[ -z "$CARGO_VER" ]]; then
  problems+=("Cargo.toml has no [workspace.package] version")
elif [[ "$CARGO_VER" != "$VER" ]]; then
  problems+=("Cargo.toml workspace version is '$CARGO_VER' but the tag is '$VER'")
fi

if [[ -z "$HEADING" ]]; then
  problems+=("CHANGELOG.md has no '## [$VER]' section")
else
  shopt -s nocasematch
  if [[ "$HEADING" =~ unreleased ]]; then
    problems+=("CHANGELOG.md section is still marked unreleased: $HEADING")
  elif [[ ! "$HEADING" =~ [0-9]{4}-[0-9]{2}-[0-9]{2} ]]; then
    problems+=("CHANGELOG.md section has no YYYY-MM-DD date: $HEADING")
  fi
  shopt -u nocasematch
fi

if ((${#problems[@]})); then
  echo "Release version mismatch for tag $TAG:" >&2
  echo "  tag:        $VER" >&2
  echo "  Cargo.toml: ${CARGO_VER:-<missing>}" >&2
  echo "  CHANGELOG:  ${HEADING:-<no section>}" >&2
  for p in "${problems[@]}"; do
    echo "error: $p" >&2
  done
  exit 1
fi

echo "OK: tag $TAG == Cargo.toml $CARGO_VER == CHANGELOG '$HEADING'"
