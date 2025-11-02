#!/bin/bash
# MyVault macOS Build Script
# This script builds MyVault for macOS and creates an app bundle

set -e

echo "Building MyVault for macOS..."

# Build for current architecture
cargo build --release

# Determine output directory
APP_NAME="MyVault"
BUNDLE_DIR="target/macos/${APP_NAME}.app"
CONTENTS_DIR="${BUNDLE_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

# Create app bundle structure
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy executable
cp "target/release/my_vault" "${MACOS_DIR}/MyVault"
chmod +x "${MACOS_DIR}/MyVault"

# Create Info.plist
cat > "${CONTENTS_DIR}/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>MyVault</string>
    <key>CFBundleIdentifier</key>
    <string>com.myvault.app</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>MyVault</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSRequiresIPhoneOS</key>
    <false/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2025 MyVault. Licensed under MIT.</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSRequiredOSVersion</key>
    <string>10.12</string>
</dict>
</plist>
EOF

echo "✅ macOS app bundle created: ${BUNDLE_DIR}"
echo "Run: open ${BUNDLE_DIR}"
