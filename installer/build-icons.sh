#!/usr/bin/env bash
#
# Regenerate platform icon bitmaps from crates/signex-app/assets/brand/signex.svg.
#
# Outputs:
#   installer/windows/signex.ico            — multi-size ICO (16..256)
#   installer/macos/Signex.icns             — macOS icon set
#   installer/linux/signex-{128,256,512}.png — Linux PNGs for .desktop files
#
# Requires one of (in order of preference):
#   - rsvg-convert (librsvg)     — best SVG fidelity
#   - magick / convert (ImageMagick 7+ or 6)
#   - inkscape
#
# On macOS, `iconutil` is used to assemble the .icns (requires Xcode CLT).
# Elsewhere, png2icns (libicns) or magick will produce the .icns.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Rasterize from signex-mark.svg (tight-cropped, guaranteed-square viewBox).
# signex.svg may have a rectangular viewBox after edits in Inkscape/Illustrator,
# which would stretch square bitmaps.
SRC="$REPO_ROOT/crates/signex-app/assets/brand/signex-mark.svg"

if [[ ! -f "$SRC" ]]; then
  echo "error: source SVG not found at $SRC" >&2
  exit 1
fi

WIN_DIR="$REPO_ROOT/installer/windows"
MAC_DIR="$REPO_ROOT/installer/macos"
LIN_DIR="$REPO_ROOT/installer/linux"
GEN_DIR="$REPO_ROOT/crates/signex-app/assets/brand/generated"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$WIN_DIR" "$MAC_DIR" "$LIN_DIR" "$GEN_DIR"

# Pick an SVG rasterizer.
# Note: on Windows, bare `convert` resolves to C:\Windows\System32\convert.exe
# (the FAT→NTFS tool), NOT ImageMagick. We detect and reject that.
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

Install one of the following:
  Windows:   winget install ImageMagick.ImageMagick   (then use 'magick')
             or:  choco install imagemagick
  macOS:     brew install librsvg imagemagick
  Linux:     apt-get install librsvg2-bin imagemagick
             or:  dnf install librsvg2-tools ImageMagick

Then re-run this script.
EOF
  exit 1
fi
echo "using rasterizer: $RASTERIZER"

render_png() {
  local size="$1"
  local out="$2"
  case "$RASTERIZER" in
    rsvg-convert)
      rsvg-convert -w "$size" -h "$size" "$SRC" -o "$out"
      ;;
    magick|convert)
      "$RASTERIZER" -background none -density 600 "$SRC" -resize "${size}x${size}" "$out"
      ;;
    inkscape)
      inkscape --export-type=png --export-width="$size" --export-height="$size" \
               --export-filename="$out" "$SRC" >/dev/null 2>&1
      ;;
  esac
}

SIZES=(16 32 48 64 128 256 512 1024)
declare -A PNGS
for s in "${SIZES[@]}"; do
  out="$TMP_DIR/signex-${s}.png"
  echo "  render ${s}×${s} -> $out"
  render_png "$s" "$out"
  PNGS[$s]="$out"
done

# Also keep a canonical 256 and 512 next to the app crate for runtime use.
cp "${PNGS[256]}" "$GEN_DIR/signex-256.png"
cp "${PNGS[512]}" "$GEN_DIR/signex-512.png"

# Windows .ico — multi-size. Prefer ImageMagick; fall back to Python + Pillow.
echo "building $WIN_DIR/signex.ico"
ICO_DONE=0
if command -v magick >/dev/null 2>&1; then
  magick "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" \
         "$WIN_DIR/signex.ico" && ICO_DONE=1
elif is_real_imagemagick_convert; then
  convert "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" \
          "$WIN_DIR/signex.ico" && ICO_DONE=1
fi
if [[ $ICO_DONE -eq 0 ]] && command -v python >/dev/null 2>&1; then
  # Build the ICO by embedding each size's natively-rendered PNG directly —
  # sharper than asking Pillow to downsample one large PNG to all sizes.
  python - "$WIN_DIR/signex.ico" \
    "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" <<'PY'
import struct, sys
from pathlib import Path

out, *srcs = sys.argv[1:]
entries = []
for p in srcs:
    data = Path(p).read_bytes()
    # The PNG's pixel dimensions are in bytes 16..24 of the IHDR chunk.
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
        w_b, h_b, 0, 0,  # width, height, colors, reserved
        1, 32,           # planes, bpp
        len(data), offset,
    )
    image_blob += data
    offset += len(data)

Path(out).write_bytes(header + dir_entries + image_blob)
print(f"  wrote {out} with {len(entries)} sizes from native-rendered PNGs")
PY
  ICO_DONE=1
fi
if [[ $ICO_DONE -eq 0 ]]; then
  echo "  skipped .ico (need magick, real ImageMagick convert, or python+Pillow)" >&2
fi

# Also stash in the generated folder for winres embedding.
cp -f "$WIN_DIR/signex.ico" "$GEN_DIR/signex.ico" 2>/dev/null || true

# macOS .icns.
echo "building $MAC_DIR/Signex.icns"
ICONSET="$TMP_DIR/Signex.iconset"
mkdir -p "$ICONSET"
# Apple iconset naming: icon_<size>x<size>[@2x].png
cp "${PNGS[16]}"   "$ICONSET/icon_16x16.png"
cp "${PNGS[32]}"   "$ICONSET/icon_16x16@2x.png"
cp "${PNGS[32]}"   "$ICONSET/icon_32x32.png"
cp "${PNGS[64]}"   "$ICONSET/icon_32x32@2x.png"
cp "${PNGS[128]}"  "$ICONSET/icon_128x128.png"
cp "${PNGS[256]}"  "$ICONSET/icon_128x128@2x.png"
cp "${PNGS[256]}"  "$ICONSET/icon_256x256.png"
cp "${PNGS[512]}"  "$ICONSET/icon_256x256@2x.png"
cp "${PNGS[512]}"  "$ICONSET/icon_512x512.png"
cp "${PNGS[1024]}" "$ICONSET/icon_512x512@2x.png"

ICNS_DONE=0
if command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns "$ICONSET" -o "$MAC_DIR/Signex.icns" && ICNS_DONE=1
elif command -v png2icns >/dev/null 2>&1; then
  png2icns "$MAC_DIR/Signex.icns" "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" \
           "${PNGS[128]}" "${PNGS[256]}" "${PNGS[512]}" && ICNS_DONE=1
elif command -v magick >/dev/null 2>&1; then
  magick "${PNGS[16]}" "${PNGS[32]}" "${PNGS[128]}" "${PNGS[256]}" "${PNGS[512]}" \
         "$MAC_DIR/Signex.icns" && ICNS_DONE=1
fi
if [[ $ICNS_DONE -eq 0 ]] && command -v python >/dev/null 2>&1; then
  python - "$MAC_DIR/Signex.icns" "${PNGS[16]}" "${PNGS[32]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" "${PNGS[512]}" "${PNGS[1024]}" <<'PY'
import sys
from PIL import Image
out, *ins = sys.argv[1:]
imgs = [Image.open(p).convert("RGBA") for p in ins]
imgs[0].save(out, format="ICNS", append_images=imgs[1:])
print(f"  pillow wrote {out}")
PY
  ICNS_DONE=1
fi
if [[ $ICNS_DONE -eq 0 ]]; then
  echo "  skipped .icns (need iconutil, png2icns, magick, or python+Pillow)" >&2
fi

# Linux PNGs for .desktop files.
echo "copying Linux PNGs to $LIN_DIR"
cp "${PNGS[128]}" "$LIN_DIR/signex-128.png"
cp "${PNGS[256]}" "$LIN_DIR/signex-256.png"
cp "${PNGS[512]}" "$LIN_DIR/signex-512.png"

echo "done."
