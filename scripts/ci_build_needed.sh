#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Prints `true` when the Rust jobs in ci.yml must run, else `false`. Inputs:
#   CI_EVENT  the GitHub event name. Unset or empty means pull_request; any
#             other event (push, merge_group, workflow_dispatch) builds.
#   CI_DRAFT  `true` on a draft PR, which skips the Rust jobs.
#   stdin     a PR's changed file paths, one per line. A PR that only
#             touches prose skips the Rust jobs.
#
# Fails closed: a path is "prose" only if it matches the list below, and an
# empty list prints `true`. Nothing under crates/ is ever prose, and neither
# are the docs files a test reads (keep PROSE_EXCEPTIONS in sync when a test
# starts reading another one).
#
#   git diff --name-only origin/trunk...HEAD | bash scripts/ci_build_needed.sh

set -euo pipefail

# An answer that needs no paths. Stdin is still read to its end, so the
# writer on the other end of the pipe never finds it closed.
answer_without_paths() {
  [ -t 0 ] || cat >/dev/null
  echo "$1"
  exit 0
}

if [ "${CI_EVENT:-pull_request}" != pull_request ]; then
  answer_without_paths true
fi
if [ "${CI_DRAFT:-}" = true ]; then
  answer_without_paths false
fi

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
