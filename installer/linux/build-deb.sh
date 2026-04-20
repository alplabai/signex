#!/bin/bash
# Build a Debian .deb package from a compiled signex binary.
# Produces: signex_<version>_<arch>.deb (Debian naming convention).
#
# Invocation:
#   installer/linux/build-deb.sh <binary_path> <version> <arch>
#
#   <arch> — "amd64" (x86_64) or "arm64" (aarch64) in Debian terminology.

set -euo pipefail

BINARY_PATH="${1:?usage: build-deb.sh <binary> <version> <arch>}"
VERSION="${2:?missing version}"
ARCH="${3:?missing arch}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

PKG_DIR="$WORK_DIR/signex_${VERSION}_${ARCH}"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$PKG_DIR/usr/share/doc/signex"

cp "$BINARY_PATH" "$PKG_DIR/usr/bin/signex"
chmod 755 "$PKG_DIR/usr/bin/signex"

# Control file — Installed-Size is approx (binary + assets); dpkg-deb
# computes the actual compressed size when it builds.
INSTALLED_SIZE=$(du -sk "$PKG_DIR/usr" | cut -f1)
cat > "$PKG_DIR/DEBIAN/control" <<CONTROL_EOF
Package: signex
Version: $VERSION
Section: electronics
Priority: optional
Architecture: $ARCH
Depends: libc6, libgcc-s1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6, libxcursor1, libxrandr2, libxi6, libxinerama1, libvulkan1
Installed-Size: $INSTALLED_SIZE
Maintainer: alpCaner <alpcaner92@gmail.com>
Homepage: https://github.com/alplabai/signex
Description: AI-first EDA editor with KiCad round-trip
 Signex is a KiCad-compatible electronics design automation editor with
 an Altium-inspired interaction layer. It opens KiCad projects, edits
 them through a faster UI, and saves them back without format drift.
CONTROL_EOF

# .desktop entry for menu integration.
cat > "$PKG_DIR/usr/share/applications/signex.desktop" <<DESKTOP_EOF
[Desktop Entry]
Type=Application
Name=Signex
Comment=AI-first EDA editor
Exec=/usr/bin/signex %F
Terminal=false
Categories=Development;Electronics;Engineering;
MimeType=application/x-kicad-schematic;application/x-kicad-project;
StartupNotify=true
DESKTOP_EOF

# README copied into the docs dir — Debian policy expects at least a
# changelog, but a short README keeps the lintian warning small.
cat > "$PKG_DIR/usr/share/doc/signex/README.Debian" <<DOC_EOF
Signex $VERSION
---------------
Installed by signex_${VERSION}_${ARCH}.deb.
Report issues at https://github.com/alplabai/signex/issues.
DOC_EOF

OUTPUT="signex_${VERSION}_${ARCH}.deb"
rm -f "$OUTPUT"

# --root-owner-group emits root:root for every file, which lintian requires.
dpkg-deb --root-owner-group --build "$PKG_DIR" "$OUTPUT"

echo "Built $OUTPUT"
