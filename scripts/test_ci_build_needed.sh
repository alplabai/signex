#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Controls for scripts/ci_build_needed.sh. A wrong `false` skips the Rust
# jobs on a PR that needed them, so every fail-closed case is pinned here.

set -euo pipefail

SCRIPT="$(dirname "$0")/ci_build_needed.sh"
failures=0

expect() {
  local want="$1" label="$2" input="$3" got
  got=$(printf '%s' "$input" | bash "$SCRIPT")
  if [ "$got" != "$want" ]; then
    echo "FAIL: $label: want $want, got $got"
    failures=$((failures + 1))
  fi
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

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "ci_build_needed.sh: all controls pass"
