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
mkdir -p "$PKG_DIR/usr/share/icons/hicolor/scalable/mimetypes"
mkdir -p "$PKG_DIR/usr/share/mime/packages"
mkdir -p "$PKG_DIR/usr/share/doc/signex"

cp "$BINARY_PATH" "$PKG_DIR/usr/bin/signex"
chmod 755 "$PKG_DIR/usr/bin/signex"

# Signex file-format icons (SVG) — Linux uses the SVG source
# directly, scaled by the desktop environment. Names follow the
# freedesktop.org MIME-icon convention
# `application-vnd.alpcaner.signex.<ext>.svg` so the
# hicolor-scalable theme pairs them with the MIME types declared
# below.
REPO_ROOT_FOR_ICONS="$(cd "$(dirname "$0")/../.." && pwd)"
FILE_ICON_DIR="$REPO_ROOT_FOR_ICONS/crates/signex-app/assets/icons/files"
for ext in snxprj snxsch snxpcb snxfpt snxsim snxlib snxsym snxpkg snxmat snxcfg snxmod; do
  src="$FILE_ICON_DIR/$ext.svg"
  if [[ -f "$src" ]]; then
    cp "$src" "$PKG_DIR/usr/share/icons/hicolor/scalable/mimetypes/application-vnd.alpcaner.signex.$ext.svg"
  fi
done

# freedesktop.org MIME XML — one glob per Signex extension.
cat > "$PKG_DIR/usr/share/mime/packages/signex.xml" <<MIME_EOF
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/vnd.alpcaner.signex.snxprj">
    <comment>Signex Project</comment>
    <glob pattern="*.snxprj"/>
    <icon name="application-vnd.alpcaner.signex.snxprj"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxsch">
    <comment>Signex Schematic</comment>
    <glob pattern="*.snxsch"/>
    <icon name="application-vnd.alpcaner.signex.snxsch"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxpcb">
    <comment>Signex PCB</comment>
    <glob pattern="*.snxpcb"/>
    <icon name="application-vnd.alpcaner.signex.snxpcb"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxfpt">
    <comment>Signex Footprint</comment>
    <glob pattern="*.snxfpt"/>
    <icon name="application-vnd.alpcaner.signex.snxfpt"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxsim">
    <comment>Signex Simulation</comment>
    <glob pattern="*.snxsim"/>
    <icon name="application-vnd.alpcaner.signex.snxsim"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxlib">
    <comment>Signex Library</comment>
    <glob pattern="*.snxlib"/>
    <icon name="application-vnd.alpcaner.signex.snxlib"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxsym">
    <comment>Signex Symbol</comment>
    <glob pattern="*.snxsym"/>
    <icon name="application-vnd.alpcaner.signex.snxsym"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxpkg">
    <comment>Signex Package</comment>
    <glob pattern="*.snxpkg"/>
    <icon name="application-vnd.alpcaner.signex.snxpkg"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxmat">
    <comment>Signex PCB Material</comment>
    <glob pattern="*.snxmat"/>
    <icon name="application-vnd.alpcaner.signex.snxmat"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxcfg">
    <comment>Signex Config</comment>
    <glob pattern="*.snxcfg"/>
    <icon name="application-vnd.alpcaner.signex.snxcfg"/>
  </mime-type>
  <mime-type type="application/vnd.alpcaner.signex.snxmod">
    <comment>Signex SPICE Model</comment>
    <glob pattern="*.snxmod"/>
    <icon name="application-vnd.alpcaner.signex.snxmod"/>
  </mime-type>
</mime-info>
MIME_EOF

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

# .desktop entry for menu integration. MimeType covers the KiCad
# handoff types plus all seven Signex native extensions declared in
# `usr/share/mime/packages/signex.xml`.
cat > "$PKG_DIR/usr/share/applications/signex.desktop" <<DESKTOP_EOF
[Desktop Entry]
Type=Application
Name=Signex
Comment=AI-first EDA editor
Exec=/usr/bin/signex %F
Terminal=false
Categories=Development;Electronics;Engineering;
MimeType=application/x-kicad-schematic;application/x-kicad-project;application/vnd.alpcaner.signex.snxprj;application/vnd.alpcaner.signex.snxsch;application/vnd.alpcaner.signex.snxpcb;application/vnd.alpcaner.signex.snxfpt;application/vnd.alpcaner.signex.snxsim;application/vnd.alpcaner.signex.snxlib;application/vnd.alpcaner.signex.snxsym;application/vnd.alpcaner.signex.snxpkg;application/vnd.alpcaner.signex.snxmat;application/vnd.alpcaner.signex.snxcfg;application/vnd.alpcaner.signex.snxmod;
StartupNotify=true
DESKTOP_EOF

# postinst / postrm refresh the MIME + icon caches so Nautilus and
# friends pick up the new .snx*** types without a logout. Both
# scripts tolerate missing tools (update-mime-database, update-desktop-database,
# gtk-update-icon-cache) silently — user may be on a minimal install.
cat > "$PKG_DIR/DEBIAN/postinst" <<'POSTINST_EOF'
#!/bin/sh
set -e
if command -v update-mime-database >/dev/null 2>&1; then
  update-mime-database /usr/share/mime >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
fi
exit 0
POSTINST_EOF
chmod 755 "$PKG_DIR/DEBIAN/postinst"

cat > "$PKG_DIR/DEBIAN/postrm" <<'POSTRM_EOF'
#!/bin/sh
set -e
if command -v update-mime-database >/dev/null 2>&1; then
  update-mime-database /usr/share/mime >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
fi
exit 0
POSTRM_EOF
chmod 755 "$PKG_DIR/DEBIAN/postrm"

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
