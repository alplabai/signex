#!/usr/bin/env bash
# What a demo recording needs (#2437), checked before anything starts:
#   check_tools.sh record   Xvfb, ffmpeg with libx264, libvpx, libwebp and
#                           libass, lavapipe, Python 3 with NumPy, and the
#                           X11 libraries the app loads (libxkbcommon-x11, ...)
#   check_tools.sh check    Python 3 only (record_all.sh --check)
# Prints the missing Debian/Ubuntu packages and exits 2 when any is missing.
# See docs/DEMO_VIDEOS.md, "Record".
set -uo pipefail

missing=()
command -v python3 >/dev/null || missing+=(python3)
if [ "${1:-record}" = record ]; then
	python3 -c 'import numpy' 2>/dev/null || missing+=(python3-numpy)
	command -v Xvfb >/dev/null || missing+=(xvfb)
	if ! command -v ffmpeg >/dev/null; then
		missing+=(ffmpeg)
	else
		encoders=$(ffmpeg -hide_banner -encoders 2>/dev/null)
		filters=$(ffmpeg -hide_banner -filters 2>/dev/null)
		for codec in libx264 libvpx-vp9 libwebp; do
			grep -qw "$codec" <<<"$encoders" || missing+=("ffmpeg with $codec")
		done
		grep -qw ass <<<"$filters" || missing+=("ffmpeg with libass")
	fi
	[ -e /usr/share/vulkan/icd.d/lvp_icd.json ] || missing+=(mesa-vulkan-drivers)
	# The X11 libraries the app's windowing loads at start, by package.
	libs=$(ldconfig -p 2>/dev/null)
	for pair in libxkbcommon-x11.so.0:libxkbcommon-x11-0 libvulkan.so.1:libvulkan1 \
		libXcursor.so.1:libxcursor1 libXi.so.6:libxi6 libXrandr.so.2:libxrandr2; do
		grep -q "${pair%%:*}" <<<"$libs" || missing+=("${pair#*:}")
	done
fi

if [ ${#missing[@]} -gt 0 ]; then
	echo "missing: ${missing[*]}" >&2
	echo "Debian/Ubuntu: sudo apt install xvfb ffmpeg mesa-vulkan-drivers python3-numpy libxkbcommon-x11-0" >&2
	exit 2
fi
