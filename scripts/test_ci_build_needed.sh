#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Controls for scripts/ci_build_needed.sh. A wrong `false` skips the Rust
# jobs on a PR that needed them, so every fail-closed case is pinned here.

set -euo pipefail

SCRIPT="$(dirname "$0")/ci_build_needed.sh"
failures=0

expect_ci() {
  local want="$1" label="$2" event="$3" draft="$4" input="$5" got
  got=$(printf '%s' "$input" | CI_EVENT="$event" CI_DRAFT="$draft" bash "$SCRIPT")
  if [ "$got" != "$want" ]; then
    echo "FAIL: $label: want $want, got $got"
    failures=$((failures + 1))
  fi
}

# A ready PR (or a local run): only the paths decide.
expect() {
  expect_ci "$1" "$2" "" "" "$3"
}

expect false "docs only" $'docs/adr/0001.md\ndocs/img/x.png\n'
expect false "changelog fragment" $'changelog.d/1485.ci.md\n'
expect false "root markdown" $'README.md\nCONTRIBUTING.md\nCHANGELOG.md\n'
expect true "empty diff fails closed" ''
expect true "crate source" $'docs/a.md\ncrates/signex-app/src/main.rs\n'
expect true "markdown under crates" $'crates/signex-gfx/tests/fonts/README.md\n'
expect true "doc read by a test" $'docs/USER_GUIDE.md\n'
expect true "command reference" $'docs/COMMANDS.md\n'
expect true "lockfile" $'Cargo.lock\n'
expect true "toolchain pin" $'rust-toolchain.toml\n'
expect true "workflow" $'.github/workflows/ci.yml\n'
expect true "script" $'scripts/check_file_size.sh\n'
expect true "move out of crates (--no-renames lists both sides)" $'crates/signex-app/src/x.rs\ndocs/x.rs\n'

# Event and draft flag. Push, merge-queue and manual runs build even with no
# or prose-only paths; a draft PR skips even when crates change.
crate=$'crates/signex-app/src/main.rs\n'
prose=$'docs/a.md\n'
expect_ci true "push builds" push "" ''
expect_ci true "push builds with prose-only paths" push "" "$prose"
expect_ci true "merge queue builds" merge_group "" ''
expect_ci true "manual run builds" workflow_dispatch "" ''
expect_ci true "a draft flag on a push is ignored" push true "$prose"
expect_ci false "draft PR skips crate changes" pull_request true "$crate"
expect_ci false "draft PR with no paths skips" pull_request true ''
expect_ci true "ready PR builds crate changes" pull_request false "$crate"
expect_ci false "ready PR skips prose" pull_request false "$prose"
expect_ci true "no draft flag reads the paths" pull_request "" "$crate"
expect_ci true "an unknown draft value reads the paths" pull_request True "$crate"

# The real pipeline, end to end: a move out of crates/ in a throwaway repo,
# listed the way ci.yml lists it. Without --no-renames git reports only the
# destination, which is prose.
repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT
git -C "$repo" init -q
mkdir -p "$repo/crates" "$repo/docs"
echo x > "$repo/crates/a.rs"
git -C "$repo" add .
git -C "$repo" -c user.name=t -c user.email=t@t commit -qm base
git -C "$repo" mv crates/a.rs docs/a.rs
git -C "$repo" -c user.name=t -c user.email=t@t commit -qm move
expect true "git mv crates/ -> docs/" \
  "$(git -C "$repo" diff --no-renames --name-only HEAD~1...HEAD)"

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "ci_build_needed.sh: all controls pass"
