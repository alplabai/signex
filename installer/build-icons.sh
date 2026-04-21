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
SRC="$REPO_ROOT/crates/signex-app/assets/brand/signex.svg"

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

# Windows .ico — multi-size.
echo "building $WIN_DIR/signex.ico"
if command -v magick >/dev/null 2>&1; then
  magick "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" \
         "$WIN_DIR/signex.ico"
elif command -v convert >/dev/null 2>&1; then
  convert "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" "${PNGS[64]}" "${PNGS[128]}" "${PNGS[256]}" \
          "$WIN_DIR/signex.ico"
else
  echo "  skipped .ico (need magick or convert)" >&2
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

if command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns "$ICONSET" -o "$MAC_DIR/Signex.icns"
elif command -v png2icns >/dev/null 2>&1; then
  png2icns "$MAC_DIR/Signex.icns" "${PNGS[16]}" "${PNGS[32]}" "${PNGS[48]}" \
           "${PNGS[128]}" "${PNGS[256]}" "${PNGS[512]}"
elif command -v magick >/dev/null 2>&1; then
  magick "${PNGS[16]}" "${PNGS[32]}" "${PNGS[128]}" "${PNGS[256]}" "${PNGS[512]}" \
         "$MAC_DIR/Signex.icns"
else
  echo "  skipped .icns (need iconutil, png2icns, or magick)" >&2
fi

# Linux PNGs for .desktop files.
echo "copying Linux PNGs to $LIN_DIR"
cp "${PNGS[128]}" "$LIN_DIR/signex-128.png"
cp "${PNGS[256]}" "$LIN_DIR/signex-256.png"
cp "${PNGS[512]}" "$LIN_DIR/signex-512.png"

echo "done."
