# MyVault Cross-Platform Deployment Plan

## Overview
This document outlines the strategy to make MyVault work on Windows, macOS, and Linux.

---

## Current Status

### ✅ Windows
- Fully implemented and tested
- Custom icon working
- Console window hidden
- All features functional
- Binary: `my_vault.exe` (10 MB)

### ⏳ macOS
- Needs implementation
- GUI framework compatible (egui)
- File operations compatible
- Encryption compatible

### ⏳ Linux
- Needs implementation
- GUI framework compatible (egui)
- File operations compatible
- Encryption compatible

---

## Phase 1: Code Changes for Cross-Platform

### 1.1 Platform-Specific Code (Already Started)

**File**: `src/platform.rs`
```rust
#[cfg(target_os = "windows")]
pub fn hide(path: &Path) -> Result<(), String> {
    // Windows-specific: hide file
}

#[cfg(target_os = "macos")]
pub fn hide(path: &Path) -> Result<(), String> {
    // macOS-specific: use extended attributes
}

#[cfg(target_os = "linux")]
pub fn hide(path: &Path) -> Result<(), String> {
    // Linux-specific: prefix with dot or xattr
}
```

### 1.2 Update Cargo.toml for Cross-Platform

```toml
[package]
name = "my_vault"
version = "1.0.0"
edition = "2021"

[dependencies]
eframe = "0.27"
rfd = "0.14"  # Cross-platform file dialog
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
argon2 = "0.5"
password-hash = { version = "0.5", features = ["alloc"] }
rand = "0.8"
zeroize = "1.7"
chacha20poly1305 = "0.10"
generic-array = "1.0"
walkdir = "2.5"
num_cpus = "1.16"
rayon = "1.8"
memmap2 = "0.9"

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winnt", "fileapi", "winbase"] }

[target.'cfg(target_os = "macos")'.dependencies]
objc = "0.2"
objc-foundation = "0.1"

[target.'cfg(target_os = "linux")'.dependencies]
xattr = "1.0"

# Desktop entry for Linux
[target.'cfg(target_os = "linux")'.build-dependencies]
```

### 1.3 Main.rs Platform Handling

```rust
#[cfg(not(debug_assertions))]
#[cfg(target_os = "windows")]
fn main() -> eframe::Result<()> {
    // Windows: hide console
    #![windows_subsystem = "windows"]
    // ... rest of main
}

#[cfg(target_os = "macos")]
fn main() -> eframe::Result<()> {
    // macOS: normal GUI
    // ... rest of main
}

#[cfg(target_os = "linux")]
fn main() -> eframe::Result<()> {
    // Linux: normal GUI
    // ... rest of main
}
```

### 1.4 File Path Handling

**Current (Windows only)**:
```rust
C:\MyVault\vault_config.json
```

**Cross-platform**:
```rust
// Windows
C:\Users\<Username>\AppData\Local\MyVault\vault_config.json

// macOS
~/Library/Application Support/MyVault/vault_config.json

// Linux
~/.local/share/myvault/vault_config.json
```

**Rust Code** (use `dirs` crate):
```toml
[dependencies]
dirs = "5.0"  # Cross-platform home directory detection
```

```rust
use dirs::config_local_dir;

fn get_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        config_local_dir().unwrap().join("MyVault")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap()
            .join("Library/Application Support/MyVault")
    }

    #[cfg(target_os = "linux")]
    {
        dirs::home_dir()
            .unwrap()
            .join(".local/share/myvault")
    }
}
```

---

## Phase 2: Platform-Specific Implementation

### 2.1 Windows (Already Done ✅)

**Executable**: `my_vault.exe`
**Installation**: Copy to any folder
**Icon**: Built-in custom lock icon
**Console**: Hidden (`windows_subsystem = "windows"`)

### 2.2 macOS (To Do)

**Executable Name**: `myvault` (no extension)
**Installation Methods**:
1. **App Bundle** (recommended)
   ```
   MyVault.app/
   ├── Contents/
   │   ├── MacOS/
   │   │   └── myvault (executable)
   │   ├── Resources/
   │   │   ├── AppIcon.icns (icon)
   │   │   └── Info.plist
   │   └── PkgInfo
   ```

2. **Homebrew** (package manager)
   ```bash
   brew install myvault
   ```

3. **Direct Binary**
   ```bash
   chmod +x myvault
   ./myvault
   ```

**Icon**: `.icns` format (convert from PNG)

**Code Signing** (optional but recommended):
```bash
codesign -s - MyVault.app
```

**Notarization** (for App Store):
```bash
xcrun altool --notarize-app \
  -f MyVault.app \
  -t osx \
  -u "apple@example.com" \
  -p "app-specific-password"
```

**File Locations**:
- Config: `~/Library/Application Support/MyVault/`
- Logs: `~/Library/Logs/MyVault/`

### 2.3 Linux (To Do)

**Executable Name**: `myvault` (no extension)
**Installation Methods**:
1. **Snap**
   ```bash
   snap install myvault
   ```

2. **Flatpak**
   ```bash
   flatpak install myvault
   ```

3. **AppImage** (portable)
   ```
   MyVault-1.0.0.AppImage
   chmod +x MyVault-1.0.0.AppImage
   ./MyVault-1.0.0.AppImage
   ```

4. **Package Manager**
   ```bash
   # Debian/Ubuntu
   apt install myvault

   # Fedora/RHEL
   dnf install myvault

   # Arch
   pacman -S myvault
   ```

**Desktop Entry**:
```
[Desktop Entry]
Name=MyVault
Exec=myvault
Icon=myvault
Type=Application
Categories=Utility;Security;
```

**File Locations**:
- Config: `~/.local/share/myvault/`
- Cache: `~/.cache/myvault/`
- Desktop Entry: `~/.local/share/applications/myvault.desktop`

---

## Phase 3: Build Configuration

### 3.1 Build Script

**File**: `build.sh` (Linux/macOS)
```bash
#!/bin/bash

# Build for current platform
cargo build --release

# Create app bundle (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    mkdir -p MyVault.app/Contents/MacOS
    mkdir -p MyVault.app/Contents/Resources
    cp target/release/myvault MyVault.app/Contents/MacOS/
    cp icon.icns MyVault.app/Contents/Resources/
    cp Info.plist MyVault.app/Contents/
fi

# Create AppImage (Linux)
if [[ "$OSTYPE" == "linux"* ]]; then
    mkdir -p AppDir/usr/bin
    mkdir -p AppDir/usr/share/applications
    cp target/release/myvault AppDir/usr/bin/
    cp myvault.desktop AppDir/usr/share/applications/
    # Use appimagetool to create AppImage
fi
```

**File**: `build.bat` (Windows)
```batch
@echo off
cargo build --release
echo Windows build complete: target\release\my_vault.exe
```

### 3.2 GitHub Actions Multi-Platform

**File**: `.github/workflows/build-multiplatform.yml`
```yaml
name: Build Multi-Platform

on: [push, pull_request]

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: actions/upload-artifact@v3
        with:
          name: my_vault-windows
          path: target/release/my_vault.exe

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: bash build-macos-app.sh
      - uses: actions/upload-artifact@v3
        with:
          name: MyVault-macos
          path: MyVault.app

  build-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: bash build-linux-appimage.sh
      - uses: actions/upload-artifact@v3
        with:
          name: MyVault-linux
          path: MyVault-*.AppImage
```

### 3.3 Release Workflow

**File**: `.github/workflows/release-multiplatform.yml`
```yaml
name: Multi-Platform Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
        with:
          files: target/release/my_vault.exe
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  release-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: bash build-macos-app.sh
      - uses: softprops/action-gh-release@v1
        with:
          files: MyVault.app.zip
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  release-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: bash build-linux-appimage.sh
      - uses: softprops/action-gh-release@v1
        with:
          files: MyVault-*.AppImage
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## Phase 4: Platform-Specific Features

### 4.1 macOS Specific

**App Bundle Info.plist**:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>myvault</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.myvault</string>
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
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2025. All rights reserved.</string>
    <key>NSMainStoryboardFile</key>
    <string>Main</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
```

**macOS-Specific Entitlements**:
- File system access
- Network access (if needed)
- Camera access (if needed)

### 4.2 Linux Specific

**Desktop Entry File**:
```
[Desktop Entry]
Version=1.0
Type=Application
Name=MyVault
Comment=Secure file encryption
Exec=/usr/bin/myvault %F
Icon=myvault
Terminal=false
Categories=Utility;Security;Cryptography;
Keywords=encryption;security;files;
```

**File Associations**:
```
MimeType=application/x-myvault;
```

**Systemd Service** (optional):
```
[Unit]
Description=MyVault Daemon
After=network.target

[Service]
Type=simple
User=myvault
ExecStart=/usr/bin/myvault-daemon

[Install]
WantedBy=multi-user.target
```

---

## Phase 5: Installation Methods by Platform

### Windows
```
1. Direct .exe download
2. Installer (WiX MSI)
3. Portable USB
4. Scoop: scoop install myvault
5. Chocolatey: choco install myvault
6. WinGet: winget install MyVault
```

### macOS
```
1. App Bundle (.app)
2. Homebrew: brew install myvault
3. Direct binary download
4. Mac App Store (future)
5. Disk Image (.dmg)
```

### Linux
```
1. AppImage (portable)
2. Snap: snap install myvault
3. Flatpak: flatpak install myvault
4. Package manager:
   - Debian: apt install myvault
   - Fedora: dnf install myvault
   - Arch: pacman -S myvault
```

---

## Phase 6: Documentation Updates

### README.md Updates
```markdown
# MyVault 🔒

Cross-platform file encryption made simple.

## Download

### Windows
- [Direct (.exe)](https://github.com/.../releases)
- [Installer (.msi)](https://github.com/.../releases)
- `scoop install myvault`
- `choco install myvault`

### macOS
- [App Bundle](https://github.com/.../releases)
- `brew install myvault`

### Linux
- [AppImage](https://github.com/.../releases)
- `snap install myvault`
- `flatpak install myvault`
```

### QUICKSTART.md Updates
```markdown
# Quick Start - MyVault

## Installation

### Windows
1. Download my_vault.exe
2. Run it
3. Done!

### macOS
1. Download MyVault.app
2. Drag to Applications
3. Run from Applications
4. Done!

### Linux
1. Download MyVault.AppImage
2. chmod +x MyVault.AppImage
3. ./MyVault.AppImage
4. Done!
```

---

## Phase 7: Testing Plan

### Testing Matrix

| Platform | Version | Architecture | GUI | File Ops | Encryption | Status |
|----------|---------|--------------|-----|----------|-----------|--------|
| Windows  | 10/11   | x86_64       | ✅  | ✅       | ✅        | ✅ Done |
| macOS    | 12+     | x86_64       | ⏳  | ⏳       | ✅        | TODO |
| macOS    | 12+     | ARM64        | ⏳  | ⏳       | ✅        | TODO |
| Linux    | All     | x86_64       | ⏳  | ⏳       | ✅        | TODO |
| Linux    | All     | ARM64        | ⏳  | ⏳       | ✅        | TODO |

### Test Cases

**For Each Platform**:
- [ ] Application launches
- [ ] UI renders correctly
- [ ] File selection works
- [ ] Encryption works
- [ ] Decryption works
- [ ] Batch operations work
- [ ] No console window (where applicable)
- [ ] Icon displays
- [ ] Config file created in correct location
- [ ] 99+ file test passes
- [ ] Error handling works

---

## Phase 8: Implementation Timeline

### Week 1-2: Windows to macOS
- [ ] Remove Windows-specific attributes
- [ ] Update platform.rs for macOS
- [ ] Add dir crate for cross-platform paths
- [ ] Update Cargo.toml dependencies
- [ ] Create macOS app bundle
- [ ] Test on macOS

### Week 3: Linux Support
- [ ] Update platform.rs for Linux
- [ ] Create desktop entry
- [ ] Create AppImage
- [ ] Test on Linux distributions
- [ ] Update installation docs

### Week 4: Automation
- [ ] Create GitHub Actions workflows
- [ ] Set up multi-platform builds
- [ ] Test automated releases
- [ ] Configure platform-specific builds

### Week 5: Polish
- [ ] Test all platforms thoroughly
- [ ] Update documentation
- [ ] Create installation guides
- [ ] Test all package managers

### Week 6: Release
- [ ] Tag v1.1.0
- [ ] Build all platforms
- [ ] Create release notes
- [ ] Publish all packages
- [ ] Announce

---

## Phase 9: Distribution Channels

### macOS Distribution
1. **Direct Download**: .app bundle from GitHub
2. **Homebrew**: Community package
3. **Mac App Store**: Official submission
4. **DMG Installer**: Traditional installer

### Linux Distribution
1. **Direct Download**: AppImage from GitHub
2. **Snap Store**: snapcraft.io
3. **Flathub**: flatpak.io
4. **Package Repos**: AUR, Ubuntu PPAs, etc.

---

## Phase 10: Code Changes Checklist

### Changes to Make

**src/main.rs**
- [x] Remove Windows subsystem attribute (make conditional)
- [ ] Use cross-platform icon
- [ ] Use cross-platform file paths

**src/platform.rs**
- [ ] Add macOS implementations
- [ ] Add Linux implementations
- [ ] Cross-platform file hiding logic

**src/config.rs**
- [ ] Use cross-platform config paths
- [ ] Handle directory creation

**Cargo.toml**
- [ ] Add `dirs` crate
- [ ] Add macOS dependencies
- [ ] Add Linux dependencies
- [ ] Add build scripts

**New Files**
- [ ] `build-macos.sh` (macOS build script)
- [ ] `build-linux.sh` (Linux build script)
- [ ] `Info.plist` (macOS app info)
- [ ] `myvault.desktop` (Linux desktop entry)

---

## Estimated Effort

| Task | Windows | macOS | Linux | Total |
|------|---------|-------|-------|-------|
| Code changes | ✅ Done | 8 hrs | 8 hrs | 16 hrs |
| Testing | ✅ Done | 4 hrs | 4 hrs | 8 hrs |
| Documentation | ✅ Done | 2 hrs | 2 hrs | 4 hrs |
| Packaging | ✅ Done | 4 hrs | 4 hrs | 8 hrs |
| CI/CD setup | ✅ Done | 4 hrs | 4 hrs | 8 hrs |
| **TOTAL** | | | | **44 hrs** |

**Timeline**: 2-3 weeks full-time, or 4-6 weeks part-time

---

## Success Criteria

- [ ] Runs on Windows 10+
- [ ] Runs on macOS 12+
- [ ] Runs on major Linux distributions
- [ ] All features work on all platforms
- [ ] All tests pass on all platforms
- [ ] Installation methods work
- [ ] Package managers work
- [ ] GitHub Actions builds succeed
- [ ] Releases available for all platforms
- [ ] Documentation complete

---

## Next Steps

1. **Create feature branch**: `git checkout -b feature/cross-platform`
2. **Make code changes** (16 hours)
3. **Test thoroughly** (8 hours)
4. **Update documentation** (4 hours)
5. **Set up CI/CD** (8 hours)
6. **Create pull request**
7. **Release as v1.1.0**

---

## Example: Quick macOS Build

```bash
# Clone repo
git clone https://github.com/username/myVault.git
cd myVault

# Build release
cargo build --release

# Create app bundle
mkdir -p MyVault.app/Contents/MacOS
mkdir -p MyVault.app/Contents/Resources
cp target/release/myvault MyVault.app/Contents/MacOS/
cp icon.icns MyVault.app/Contents/Resources/
cp Info.plist MyVault.app/Contents/

# Run
open MyVault.app
```

---

## Example: Quick Linux Build

```bash
# Clone repo
git clone https://github.com/username/myVault.git
cd myVault

# Build release
cargo build --release

# Make executable
chmod +x target/release/myvault

# Run
./target/release/myvault
```

---

**Status**: Cross-platform support planned and ready for implementation

**Estimated Completion**: 2-3 weeks

Let's make MyVault truly cross-platform! 🚀
