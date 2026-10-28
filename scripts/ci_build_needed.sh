#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Reads changed file paths on stdin, one per line, and prints `true` when
# any of them can change what the Rust jobs in ci.yml build or test, else
# `false`. ci.yml uses it to skip compile/test on PRs that only touch prose.
#
# Fails closed: a path is "prose" only if it matches the list below, and an
# empty list prints `true`. Nothing under crates/ is ever prose, and neither
# are the docs files a test reads (keep PROSE_EXCEPTIONS in sync when a test
# starts reading another one).
#
#   git diff --name-only origin/trunk...HEAD | bash scripts/ci_build_needed.sh

set -euo pipefail

# Docs read by signex-app's user_guide and command_reference tests.
PROSE_EXCEPTIONS='^docs/(USER_GUIDE|COMMANDS)\.md$'
PROSE='^(docs/|changelog\.d/|\.github/ISSUE_TEMPLATE/)|^[^/]+\.md$'

seen=0
while IFS= read -r path; do
  [ -z "$path" ] && continue
  seen=1
  if [[ "$path" =~ $PROSE_EXCEPTIONS ]] || ! [[ "$path" =~ $PROSE ]]; then
    echo true
    exit 0
  fi
done

if [ "$seen" -eq 0 ]; then
  echo true
else
  echo false
fi
