#!/usr/bin/env bash
#
# Regenerate per-file-type icon bitmaps for Signex's native `.snx***`
# extensions from the SVG sources under
# `crates/signex-app/assets/icons/files/`.
#
# One SVG per file type — seven total:
#   snxprj  Signex project
#   snxsch  Signex schematic
#   snxpcb  Signex PCB
#   snxfpt  Signex footprint
#   snxsim  Signex simulation
#   snxlib  Signex library
#   snxsym  Signex symbol
#
# Outputs (per file type):
#   installer/windows/files/<ext>.ico            — multi-size ICO (16..256)
#   installer/macos/files/<ext>.icns             — macOS icon set
#   installer/linux/files/<ext>.svg              — Linux copies the SVG verbatim
#
# Requires one of (in order of preference):
#   - rsvg-convert (librsvg)      — best SVG fidelity
#   - magick / convert (ImageMagick 7+ or 6)
#   - inkscape
#
# On macOS, `iconutil` is used to assemble the .icns (requires Xcode CLT).
# Elsewhere, png2icns (libicns), magick, or Python + Pillow will produce it.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$REPO_ROOT/crates/signex-app/assets/icons/files"

if [[ ! -d "$SRC_DIR" ]]; then
  echo "error: source SVG dir not found at $SRC_DIR" >&2
  exit 1
fi

WIN_DIR="$REPO_ROOT/installer/windows/files"
MAC_DIR="$REPO_ROOT/installer/macos/files"
LIN_DIR="$REPO_ROOT/installer/linux/files"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$WIN_DIR" "$MAC_DIR" "$LIN_DIR"

# Pick an SVG rasterizer. Same detection logic as the main
# `build-icons.sh` — see its comments for why bare `convert` is
# gated on Windows.
is_real_imagemagick_convert() {
  local path
  path="$(command -v convert 2>/dev/null || true)"
  [[ -z "$path" ]] && return 1
  case "$path" in
    /c/Windows/*|/c/WINDOWS/*|*\\Windows\\*|*\\WINDOWS\\*) return 1 ;;
  esac
  convert --version 2>&1 | grep -qi "ImageMagick"
}

RASTERIZER=""
if command -v rsvg-convert >/dev/null 2>&1; then
  RASTERIZER="rsvg-convert"
elif command -v magick >/dev/null 2>&1; then
  RASTERIZER="magick"
elif is_real_imagemagick_convert; then
  RASTERIZER="convert"
elif command -v inkscape >/dev/null 2>&1; then
  RASTERIZER="inkscape"
else
  cat >&2 <<EOF
error: no SVG rasterizer found on PATH.

Install one of:
  Windows:   winget install ImageMagick.ImageMagick
  macOS:     brew install librsvg imagemagick
  Linux:     apt-get install librsvg2-bin imagemagick
EOF
  exit 1
fi
echo "using rasterizer: $RASTERIZER"

render_png() {
  local src="$1" size="$2" out="$3"
  case "$RASTERIZER" in
    rsvg-convert)
      rsvg-convert -w "$size" -h "$size" "$src" -o "$out"
      ;;
    magick|convert)
      "$RASTERIZER" -background none -density 600 "$src" -resize "${size}x${size}" "$out"
      ;;
    inkscape)
      inkscape --export-type=png --export-width="$size" --export-height="$size" \
               --export-filename="$out" "$src" >/dev/null 2>&1
      ;;
  esac
}

build_ico() {
  local ext="$1" out_ico="$2"; shift 2
  local pngs=("$@")
  if command -v magick >/dev/null 2>&1; then
    magick "${pngs[@]}" "$out_ico"
  elif is_real_imagemagick_convert; then
    convert "${pngs[@]}" "$out_ico"
  elif command -v python >/dev/null 2>&1; then
    python - "$out_ico" "${pngs[@]}" <<'PY'
import struct, sys
from pathlib import Path
out, *srcs = sys.argv[1:]
entries = []
for p in srcs:
    data = Path(p).read_bytes()
    w = int.from_bytes(data[16:20], "big")
    h = int.from_bytes(data[20:24], "big")
    entries.append((w, h, data))
header = struct.pack("<HHH", 0, 1, len(entries))
dir_entries = b""
image_blob  = b""
offset = 6 + 16 * len(entries)
for (w, h, data) in entries:
    w_b = 0 if w >= 256 else w
    h_b = 0 if h >= 256 else h
    dir_entries += struct.pack(
        "<BBBBHHII",
        w_b, h_b, 0, 0,
        1, 32,
        len(data), offset,
    )
    image_blob += data
    offset += len(data)
Path(out).write_bytes(header + dir_entries + image_blob)
PY
  else
    echo "  skipped .ico for $ext (need magick / real convert / python+Pillow)" >&2
    return 1
  fi
}

build_icns() {
  local ext="$1" out_icns="$2" iconset="$3"
  if command -v iconutil >/dev/null 2>&1; then
    iconutil -c icns "$iconset" -o "$out_icns"
  elif command -v png2icns >/dev/null 2>&1; then
    png2icns "$out_icns" "$iconset"/icon_{16x16,32x32,128x128,256x256,512x512}.png
  elif command -v magick >/dev/null 2>&1; then
    magick "$iconset"/icon_{16x16,32x32,128x128,256x256,512x512}.png "$out_icns"
  elif command -v python >/dev/null 2>&1; then
    python - "$out_icns" "$iconset"/icon_16x16.png "$iconset"/icon_32x32.png \
                         "$iconset"/icon_32x32@2x.png "$iconset"/icon_128x128.png \
                         "$iconset"/icon_256x256.png "$iconset"/icon_512x512.png \
                         "$iconset"/icon_512x512@2x.png <<'PY'
import sys
from PIL import Image
out, *ins = sys.argv[1:]
imgs = [Image.open(p).convert("RGBA") for p in ins]
imgs[0].save(out, format="ICNS", append_images=imgs[1:])
PY
  else
    echo "  skipped .icns for $ext (need iconutil / png2icns / magick / python+Pillow)" >&2
    return 1
  fi
}

SIZES=(16 32 48 64 128 256 512 1024)

for ext in snxprj snxsch snxpcb snxfpt snxsim snxlib snxsym snxpkg snxmat snxcfg snxmod; do
  src="$SRC_DIR/$ext.svg"
  if [[ ! -f "$src" ]]; then
    echo "warn: missing $src — skipping $ext" >&2
    continue
  fi

  echo "=== $ext ==="

  # Linux is a straight SVG copy. Scaled-on-the-fly by the desktop
  # environment; no rasterization needed.
  cp -f "$src" "$LIN_DIR/$ext.svg"

  # Render every size into a temp dir for this extension.
  declare -A PNGS
  ext_tmp="$TMP_DIR/$ext"
  mkdir -p "$ext_tmp"
  for s in "${SIZES[@]}"; do
    out="$ext_tmp/${s}.png"
    echo "  render ${s}×${s}"
    render_png "$src" "$s" "$out"
    PNGS[$s]="$out"
  done

  # Windows .ico — 16/32/48/64/128/256 embedded.
  echo "  building $WIN_DIR/$ext.ico"
  build_ico "$ext" "$WIN_DIR/$ext.ico" \
    "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" \
    "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" || true

  # macOS .icns — Apple iconset naming (icon_<size>x<size>[@2x].png).
  iconset="$ext_tmp/$ext.iconset"
  mkdir -p "$iconset"
  cp "${PNGS[16]}"   "$iconset/icon_16x16.png"
  cp "${PNGS[32]}"   "$iconset/icon_16x16@2x.png"
  cp "${PNGS[32]}"   "$iconset/icon_32x32.png"
  cp "${PNGS[64]}"   "$iconset/icon_32x32@2x.png"
  cp "${PNGS[128]}"  "$iconset/icon_128x128.png"
  cp "${PNGS[256]}"  "$iconset/icon_128x128@2x.png"
  cp "${PNGS[256]}"  "$iconset/icon_256x256.png"
  cp "${PNGS[512]}"  "$iconset/icon_256x256@2x.png"
  cp "${PNGS[512]}"  "$iconset/icon_512x512.png"
  cp "${PNGS[1024]}" "$iconset/icon_512x512@2x.png"
  echo "  building $MAC_DIR/$ext.icns"
  build_icns "$ext" "$MAC_DIR/$ext.icns" "$iconset" || true

  unset PNGS
done

echo "done."
