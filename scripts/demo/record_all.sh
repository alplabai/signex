#!/usr/bin/env bash
# Record every demo story into docs/media/ and cut the website hero loop (#2437).
#   scripts/demo/record_all.sh            record (needs Xvfb, ffmpeg, lavapipe)
#   scripts/demo/record_all.sh --check    run every story headless, no display
# Build first: cargo build --release -p signex-app -p signex-cli
# One recording at a time (a lock file); the encodes run under nice.
# See docs/DEMO_VIDEOS.md.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out=${DEMO_OUT:-$repo/docs/media}
export SIGNEX_BIN=${SIGNEX_BIN:-${CARGO_TARGET_DIR:-$repo/target}/release/signex}
export SIGNEX_CLI=${SIGNEX_CLI:-${CARGO_TARGET_DIR:-$repo/target}/release/signex-cli}
stories=(schematic-capture board-layout automation)

missing=()
command -v python3 >/dev/null || missing+=(python3)
python3 -c 'import numpy' 2>/dev/null || missing+=("python3-numpy")
[ -x "$SIGNEX_CLI" ] || { echo "no $SIGNEX_CLI: cargo build --release -p signex-app -p signex-cli" >&2; exit 2; }

if [ "${1:-}" = --check ]; then
	[ ${#missing[@]} -eq 0 ] || { echo "missing: ${missing[*]}" >&2; exit 2; }
	for story in "${stories[@]}"; do
		work=$(mktemp -d "${TMPDIR:-/tmp}/signex-demo-check.XXXXXX")
		echo "== $story (headless)" >&2
		python3 "$here/drive_story.py" prepare "$here/stories/$story.py" --work "$work" >/dev/null
		python3 "$here/drive_story.py" run "$here/stories/$story.py" --work "$work"
		rm -rf "$work"
	done
	exit 0
fi

command -v Xvfb >/dev/null || missing+=("xvfb")
command -v ffmpeg >/dev/null || missing+=("ffmpeg")
[ -e /usr/share/vulkan/icd.d/lvp_icd.json ] || missing+=("mesa-vulkan-drivers")
[ -x "$SIGNEX_BIN" ] || { echo "no $SIGNEX_BIN: cargo build --release -p signex-app" >&2; exit 2; }
if [ ${#missing[@]} -gt 0 ]; then
	echo "missing: ${missing[*]} (Debian/Ubuntu: sudo apt install ${missing[*]})" >&2
	exit 2
fi

exec 9>"${TMPDIR:-/tmp}/signex-demo.lock"
flock -n 9 || { echo "another recording is running" >&2; exit 2; }

mkdir -p "$out"
for story in "${stories[@]}"; do
	"$here/record_story.sh" "$here/stories/$story.py" "$out"
done
echo "== hero loop" >&2
nice -n 10 python3 "$here/render.py" hero --schematic "$out/.schematic-capture.clean.mp4" \
	--board "$out/.board-layout.clean.mp4" --out "$out"
rm -f "$out"/.*.clean.mp4
ls -l "$out"
