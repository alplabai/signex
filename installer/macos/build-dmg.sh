#!/bin/bash
# Build a macOS DMG installer from a compiled signex binary.
#
# Invocation from CI:
#   installer/macos/build-dmg.sh <binary_path> <version> <arch>
#
#   <binary_path> — path to the compiled signex binary
#   <version>     — version string without the leading "v"
#   <arch>        — "aarch64" or "x86_64"
#
# Output: signex-macos-<arch>-<version>.dmg in the CWD.

set -euo pipefail

BINARY_PATH="${1:?usage: build-dmg.sh <binary> <version> <arch>}"
VERSION="${2:?missing version}"
ARCH="${3:?missing arch}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

APP_BUNDLE="$WORK_DIR/Signex.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

mkdir -p "$MACOS" "$RESOURCES"

# Binary goes into MacOS/, named exactly what Info.plist CFBundleExecutable says.
cp "$BINARY_PATH" "$MACOS/signex"
chmod +x "$MACOS/signex"

# Info.plist with version substituted.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
sed "s/__VERSION__/$VERSION/g" "$SCRIPT_DIR/Info.plist" > "$CONTENTS/Info.plist"

# Optional icon — drop a Signex.icns next to build-dmg.sh to include it.
if [[ -f "$SCRIPT_DIR/Signex.icns" ]]; then
    cp "$SCRIPT_DIR/Signex.icns" "$RESOURCES/Signex.icns"
    # Patch Info.plist to reference it.
    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string Signex" "$CONTENTS/Info.plist" || true
fi

# Assemble the DMG contents: the .app plus a symlink to /Applications so
# the user can drag-and-drop to install.
DMG_STAGING="$WORK_DIR/dmg-staging"
mkdir -p "$DMG_STAGING"
cp -R "$APP_BUNDLE" "$DMG_STAGING/Signex.app"
ln -s /Applications "$DMG_STAGING/Applications"

OUTPUT="signex-macos-$ARCH-$VERSION.dmg"
rm -f "$OUTPUT"

hdiutil create \
    -volname "Signex $VERSION" \
    -srcfolder "$DMG_STAGING" \
    -ov \
    -format UDZO \
    "$OUTPUT"

echo "Built $OUTPUT"
