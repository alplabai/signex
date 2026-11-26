#!/usr/bin/env bash
# Record one demo story (#2437): a virtual display, the release app with agent
# connections on, an ffmpeg capture, the story's steps through signex-cli, then
# render.py. Usage: record_story.sh <stories/name.py> <out dir>
# Environment: SIGNEX_BIN, SIGNEX_CLI (release binaries), DEMO_SIZE (1920x1080),
# DEMO_UI_SCALE (1.25), DEMO_KEEP_WORK=1 keeps the work folder.
# See docs/DEMO_VIDEOS.md.
set -euo pipefail

story=$(realpath "$1")
out=$(realpath -m "$2")
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
name=$(basename "$story" .py)
size=${DEMO_SIZE:-1920x1080}
width=${size%x*}
height=${size#*x}
bin=${SIGNEX_BIN:-$repo/target/release/signex}
cli=${SIGNEX_CLI:-$repo/target/release/signex-cli}

# Fail before anything starts when a tool is missing, naming its package.
"$here/check_tools.sh" record
for built in "$bin" "$cli"; do
	[ -x "$built" ] || { echo "no $built: cargo build --release -p signex-app -p signex-cli" >&2; exit 2; }
done

work=$(mktemp -d "${TMPDIR:-/tmp}/signex-demo-$name.XXXXXX")
xvfb_pid="" app_pid="" ffmpeg_pid=""
cleanup() {
	[ -n "$ffmpeg_pid" ] && kill -INT "$ffmpeg_pid" 2>/dev/null && wait "$ffmpeg_pid" 2>/dev/null
	# Only the processes this script started; the user's own Signex is never touched.
	[ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null && sleep 2 && kill -9 "$app_pid" 2>/dev/null
	[ -n "$xvfb_pid" ] && kill "$xvfb_pid" 2>/dev/null
	if [ "${DEMO_KEEP_WORK:-0}" != 1 ]; then rm -rf "$work"; fi
	return 0
}
trap cleanup EXIT

# A private config root and runtime folder: the window and signex-cli meet in
# the same agent registry, and the user's own preferences are never read.
mkdir -p "$work/config/signex" "$work/runtime"
chmod 700 "$work/runtime"
cat >"$work/config/signex/prefs.json" <<EOF
{
  "agent_connections_enabled": true,
  "first_run_tour_dismissed": true,
  "theme": "graphite_dark",
  "ui_scale": ${DEMO_UI_SCALE:-1.25},
  "remember_window_geometry": true,
  "autosave_enabled": false,
  "dock": {
    "left": {"panels": ["projects"], "collapsed": false},
    "right": {"panels": ["properties"], "collapsed": false},
    "bottom": {"panels": ["messages"], "collapsed": true}
  }
}
EOF
cat >"$work/config/signex/window_geometry.json" <<EOF
{"version": 1, "monitor_name": null, "monitor_index": 0, "x": 0, "y": 0,
 "width": $width, "height": $height, "maximized": false}
EOF
export XDG_CONFIG_HOME="$work/config" XDG_RUNTIME_DIR="$work/runtime"

echo "== $name: set-up" >&2
mapfile -t open_paths < <(python3 "$here/drive_story.py" prepare "$story" --work "$work" --cli "$cli")

# A free display number.
display=90
while [ -e "/tmp/.X11-unix/X$display" ] || [ -e "/tmp/.X$display-lock" ]; do display=$((display + 1)); done
Xvfb ":$display" -screen 0 "${size}x24" -nolisten tcp -dpi 96 >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$display" ] && break; sleep 0.1; done
export DISPLAY=":$display"

# Software Vulkan (lavapipe) for wgpu; nothing here needs a GPU.
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LIBGL_ALWAYS_SOFTWARE=1
echo "== $name: starting Signex" >&2
"$bin" "${open_paths[@]}" >"$work/signex.out" 2>"$work/signex.log" &
app_pid=$!

# Ready when the project's registry entry names the agent socket.
ready=0
for _ in $(seq 180); do
	if grep -qs '"socket": *"/' "$work"/runtime/signex-agent/registry/*.json; then ready=1; break; fi
	kill -0 "$app_pid" 2>/dev/null || break
	sleep 0.5
done
if [ "$ready" != 1 ]; then
	echo "Signex did not open the agent socket; log: $work/signex.log" >&2
	DEMO_KEEP_WORK=1
	exit 1
fi
sleep 4 # the first frames under software rendering

capture_start=$(date +%s.%N)
ffmpeg -v error -f x11grab -draw_mouse 0 -framerate 15 -video_size "$size" -i ":$display" \
	-c:v libx264 -preset ultrafast -crf 14 -pix_fmt yuv420p "$work/raw.mkv" &
ffmpeg_pid=$!
sleep 3

echo "== $name: steps" >&2
python3 "$here/drive_story.py" run "$story" --work "$work" --cli "$cli" --attach \
	--log "$work/steps.json" --capture-start "$capture_start"
sleep 1
kill -INT "$ffmpeg_pid"
wait "$ffmpeg_pid" || true
ffmpeg_pid=""

echo "== $name: render" >&2
title=$(python3 "$here/drive_story.py" title "$story")
nice -n 10 python3 "$here/render.py" story --raw "$work/raw.mkv" --steps "$work/steps.json" \
	--title "$title" --out "$out" --name "$name"
# The captionless cut, for the hero loop.
cp "$work/$name.clean.mp4" "$out/.$name.clean.mp4"
