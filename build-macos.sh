#!/bin/bash
# MyVault macOS build script.
#
# Produces a universal (Apple Silicon + Intel) .app bundle with its icon and a
# code signature.
#
# Usage:
#   ./build-macos.sh                 # universal build, ad-hoc signature
#   ./build-macos.sh --arch host     # build only for this machine's architecture
#   CODESIGN_IDENTITY="Developer ID Application: Name (TEAMID)" ./build-macos.sh
#
# Ad-hoc signing (the default) lets the app run on THIS machine but still shows
# "unidentified developer" on other machines. For distribution, pass a real
# Developer ID via CODESIGN_IDENTITY; the script then also enables the hardened
# runtime, which notarization requires.

set -euo pipefail
cd "$(dirname "$0")"

# ---- configuration -------------------------------------------------------
APP_NAME="MyVault"
BINARY="my_vault"                       # cargo's artifact name (Cargo.toml [package] name)
IDENTIFIER="com.myvault.app"
VERSION="1.0.0"
ICON_SRC="resources/icon.icns"
MIN_MACOS="10.12"

OUT_DIR="target/macos"
BUNDLE_DIR="${OUT_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${BUNDLE_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

ARCH_MODE="universal"                   # universal | host
[ "${1:-}" = "--arch" ] && ARCH_MODE="${2:-universal}"

# ---- build the executable ------------------------------------------------
build_for() {
    local target="$1"
    if ! rustup target list --installed | grep -qx "$target"; then
        echo "Installing Rust target ${target}..."
        rustup target add "$target"
    fi
    echo "Building for ${target}..."
    cargo build --release --target "$target"
}

echo "== Building ${APP_NAME} (${ARCH_MODE}) =="
if [ "$ARCH_MODE" = "host" ]; then
    cargo build --release
    BIN_PATH="target/release/${BINARY}"
else
    build_for aarch64-apple-darwin
    build_for x86_64-apple-darwin
    mkdir -p "$OUT_DIR"
    BIN_PATH="${OUT_DIR}/${BINARY}-universal"
    echo "Merging into a universal binary..."
    lipo -create -output "$BIN_PATH" \
        "target/aarch64-apple-darwin/release/${BINARY}" \
        "target/x86_64-apple-darwin/release/${BINARY}"
fi

# ---- assemble the .app bundle --------------------------------------------
echo "Assembling ${BUNDLE_DIR}..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BIN_PATH" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

# Icon: copy it and reference it, so the bundle actually shows it.
ICON_PLIST_ENTRY=""
if [ -f "$ICON_SRC" ]; then
    cp "$ICON_SRC" "${RESOURCES_DIR}/icon.icns"
    ICON_PLIST_ENTRY="    <key>CFBundleIconFile</key>
    <string>icon.icns</string>"
else
    echo "WARNING: ${ICON_SRC} not found; bundle will use a blank icon."
fi

cat > "${CONTENTS_DIR}/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${IDENTIFIER}</string>
${ICON_PLIST_ENTRY}
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>${MIN_MACOS}</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2025 MyVault. Licensed under MIT.</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

# ---- code signing --------------------------------------------------------
if [ -n "${CODESIGN_IDENTITY:-}" ]; then
    echo "Signing with Developer ID: ${CODESIGN_IDENTITY}"
    codesign --force --deep --options runtime --timestamp \
        --sign "$CODESIGN_IDENTITY" "$BUNDLE_DIR"
else
    echo "Signing ad-hoc (runs on this machine; set CODESIGN_IDENTITY to distribute)"
    codesign --force --deep --sign - "$BUNDLE_DIR"
fi

# ---- verify --------------------------------------------------------------
echo "== Verifying =="
lipo -info "${MACOS_DIR}/${APP_NAME}" || true
codesign --verify --deep --strict --verbose=1 "$BUNDLE_DIR"

echo ""
echo "Built ${BUNDLE_DIR}"
echo "Run:  open \"${BUNDLE_DIR}\""
