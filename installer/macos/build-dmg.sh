#!/bin/bash
# Build a macOS DMG installer from a compiled signex binary.
#
# Invocation from CI:
#   installer/macos/build-dmg.sh <binary_path> <version> <arch>
#
#   <binary_path> — path to the compiled signex binary
#   <version>     — version string without the leading "v"
#   <arch>        — filename label only ("universal" in CI)
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

# Stock library -> Contents/Resources/library (the app resolves it as
# <exe dir>/../Resources/library). Skipped on a clone without the assets.
# Copied before codesign so it is sealed into the bundle.
STOCK_LIB="$(cd "$SCRIPT_DIR/../.." && pwd)/assets/stock-library"
if [[ -f "$STOCK_LIB/Stock.snxlib" ]]; then
    mkdir -p "$RESOURCES/library"
    cp -R "$STOCK_LIB/." "$RESOURCES/library/"
    find "$RESOURCES/library" \( -name .git -o -name .gitkeep \) -exec rm -rf {} +
fi

# Licenses of the fonts compiled into the binary (#1179). Before codesign,
# like the stock library, so they are sealed into the bundle.
bash "$SCRIPT_DIR/../copy-font-licenses.sh" "$RESOURCES/licenses/fonts"

# Sign the bundle.
#
# With MACOS_SIGN_IDENTITY set (release.yml imports a Developer ID
# Application certificate into a temporary keychain, MACOS_KEYCHAIN) the
# bundle is signed for distribution: hardened runtime + secure timestamp,
# both required for notarization. Nested code is signed inside-out (the
# executable first, then the bundle) instead of `--deep`, which Apple
# deprecates for signing. No entitlements file is passed: Signex does no
# JIT, loads no unsigned plugins and uses Metal/wgpu without any
# hardened-runtime exception. If that ever changes, set
# MACOS_ENTITLEMENTS=<plist> and it is applied to both signatures.
#
# Without MACOS_SIGN_IDENTITY, fall back to an ad-hoc signature.
# Apple Silicon (arm64) macOS refuses to launch any executable that
# isn't at least ad-hoc signed ("Signex is damaged and can't be opened",
# issue #49). Ad-hoc doesn't vouch for origin, so users still need
# right-click -> Open on first launch.
# Apple's timestamp service is occasionally unreachable; retry a few times.
codesign_retry() {
    local i
    for i in 1 2 3; do
        if codesign "$@"; then return 0; fi
        echo "codesign attempt $i failed; retrying" >&2
        sleep $((5 * i))
    done
    return 1
}

SIGN_IDENTITY="${MACOS_SIGN_IDENTITY:-}"
CODESIGN_ARGS=(--force)
if [[ -n "$SIGN_IDENTITY" ]]; then
    CODESIGN_ARGS+=(--options runtime --timestamp --sign "$SIGN_IDENTITY")
    if [[ -n "${MACOS_KEYCHAIN:-}" ]]; then
        CODESIGN_ARGS+=(--keychain "$MACOS_KEYCHAIN")
    fi
    if [[ -n "${MACOS_ENTITLEMENTS:-}" ]]; then
        CODESIGN_ARGS+=(--entitlements "$MACOS_ENTITLEMENTS")
    fi
    codesign_retry "${CODESIGN_ARGS[@]}" "$MACOS/signex"
    codesign_retry "${CODESIGN_ARGS[@]}" "$APP_BUNDLE"
    codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
else
    codesign --force --deep --sign - "$APP_BUNDLE"
fi

# Assemble the DMG contents: the .app plus a symlink to /Applications so
# the user can drag-and-drop to install.
DMG_STAGING="$WORK_DIR/dmg-staging"
mkdir -p "$DMG_STAGING"
cp -R "$APP_BUNDLE" "$DMG_STAGING/Signex.app"
ln -s /Applications "$DMG_STAGING/Applications"

OUTPUT="signex-macos-$ARCH-$VERSION.dmg"
rm -f "$OUTPUT"

# Size the image explicitly. hdiutil's own estimate for -srcfolder runs
# short once the bundle carries a universal binary and the stock library
# ("No space left on device" while copying Contents/MacOS/signex). Staging
# size plus 20% and 32 MB covers HFS+ metadata; UDZO compresses the result,
# so the slack does not reach the shipped .dmg.
STAGING_KB=$(du -sk "$DMG_STAGING" | cut -f1)
DMG_SIZE_MB=$(( STAGING_KB / 1024 * 6 / 5 + 32 ))

hdiutil create \
    -volname "Signex $VERSION" \
    -srcfolder "$DMG_STAGING" \
    -size "${DMG_SIZE_MB}m" \
    -fs HFS+ \
    -ov \
    -format UDZO \
    "$OUTPUT"

# Sign the disk image itself too (Developer ID builds only); notarization
# of the DMG and `stapler staple` happen in release.yml afterwards.
if [[ -n "$SIGN_IDENTITY" ]]; then
    DMG_SIGN_ARGS=(--force --timestamp --sign "$SIGN_IDENTITY")
    if [[ -n "${MACOS_KEYCHAIN:-}" ]]; then
        DMG_SIGN_ARGS+=(--keychain "$MACOS_KEYCHAIN")
    fi
    codesign_retry "${DMG_SIGN_ARGS[@]}" "$OUTPUT"
    codesign --verify --strict --verbose=2 "$OUTPUT"
fi

echo "Built $OUTPUT"
