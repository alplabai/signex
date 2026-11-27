#!/usr/bin/env bash
# Record every demo story into docs/media/ and cut the website hero loop (#2437).
#   scripts/demo/record_all.sh            record (needs Xvfb, ffmpeg, lavapipe)
#   scripts/demo/record_all.sh --check    run every story headless, no display
# Build first: cargo build --release -p signex-app -p signex-cli
# A story that sets PENDING is skipped with its reason. One recording at a
# time (a lock file); the encodes run under nice. See docs/DEMO_VIDEOS.md.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out=${DEMO_OUT:-$repo/docs/media}
export SIGNEX_BIN=${SIGNEX_BIN:-${CARGO_TARGET_DIR:-$repo/target}/release/signex}
export SIGNEX_CLI=${SIGNEX_CLI:-${CARGO_TARGET_DIR:-$repo/target}/release/signex-cli}
stories=(schematic-capture board-layout automation design-notebook block-diagram routing-fab)

# The reason a story is pending, or nothing when it is ready.
pending() {
	python3 "$here/drive_story.py" pending "$here/stories/$1.py"
}

if [ "${1:-}" = --check ]; then
	"$here/check_tools.sh" check
	[ -x "$SIGNEX_CLI" ] || { echo "no $SIGNEX_CLI: cargo build --release -p signex-cli" >&2; exit 2; }
	failed=()
	for story in "${stories[@]}"; do
		why=$(pending "$story")
		if [ -n "$why" ]; then
			echo "== $story: skipped, pending: $why" >&2
			continue
		fi
		work=$(mktemp -d "${TMPDIR:-/tmp}/signex-demo-check.XXXXXX")
		echo "== $story (headless)" >&2
		if python3 "$here/drive_story.py" prepare "$here/stories/$story.py" --work "$work" >/dev/null &&
			python3 "$here/drive_story.py" run "$here/stories/$story.py" --work "$work"; then
			echo "== $story: ok" >&2
		else
			failed+=("$story")
		fi
		rm -rf "$work"
	done
	if [ ${#failed[@]} -gt 0 ]; then
		echo "failed: ${failed[*]}" >&2
		exit 1
	fi
	exit 0
fi

"$here/check_tools.sh" record
[ -x "$SIGNEX_CLI" ] || { echo "no $SIGNEX_CLI: cargo build --release -p signex-app -p signex-cli" >&2; exit 2; }
[ -x "$SIGNEX_BIN" ] || { echo "no $SIGNEX_BIN: cargo build --release -p signex-app" >&2; exit 2; }

exec 9>"${TMPDIR:-/tmp}/signex-demo.lock"
flock -n 9 || { echo "another recording is running" >&2; exit 2; }

mkdir -p "$out"
for story in "${stories[@]}"; do
	why=$(pending "$story")
	if [ -n "$why" ]; then
		echo "== $story: skipped, pending: $why" >&2
		continue
	fi
	"$here/record_story.sh" "$here/stories/$story.py" "$out"
done
echo "== hero loop" >&2
nice -n 10 python3 "$here/render.py" hero --schematic "$out/.schematic-capture.clean.mp4" \
	--board "$out/.board-layout.clean.mp4" --out "$out"
rm -f "$out"/.*.clean.mp4
ls -l "$out"
