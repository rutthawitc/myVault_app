#!/bin/bash
# MyVault Linux Build Script
# This script builds MyVault for Linux and creates AppImage, Snap, and Flatpak packages

set -e

echo "Building MyVault for Linux..."

# Build release binary
cargo build --release

BINARY_PATH="target/release/my_vault"
VERSION="1.0.0"

# Create AppImage directory structure
APPDIR="target/linux/MyVault.AppDir"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Copy executable
cp "${BINARY_PATH}" "${APPDIR}/usr/bin/myvault"
chmod +x "${APPDIR}/usr/bin/myvault"

# Create desktop entry
cat > "${APPDIR}/usr/share/applications/myvault.desktop" << 'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=MyVault
Comment=Secure file encryption made simple
Exec=myvault
Icon=myvault
Terminal=false
Categories=Utility;Security;
StartupNotify=true
X-AppImage-Version=1.0.0
EOF

# Create AppRun (entry point for AppImage)
cat > "${APPDIR}/AppRun" << 'EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE="${SELF%/*}"
EXEC="${HERE}/usr/bin/myvault"
exec "$EXEC" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

echo "✅ AppImage structure created: ${APPDIR}"
echo "Note: appimagetool is required to create .AppImage from directory"
echo "Command: appimagetool ${APPDIR} MyVault-${VERSION}-x86_64.AppImage"
