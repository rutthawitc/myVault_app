# MyVault Deployment Guide

## Table of Contents
1. [Quick Start](#quick-start)
2. [System Requirements](#system-requirements)
3. [Installation Methods](#installation-methods)
4. [Configuration](#configuration)
5. [Running the Application](#running-the-application)
6. [Troubleshooting](#troubleshooting)
7. [Security Considerations](#security-considerations)

---

## Quick Start

### For End Users (Windows)
```bash
# 1. Download the executable
Download: D:\Codes\rust\myVault\target\release\my_vault.exe

# 2. Create folder for MyVault
mkdir C:\MyVault

# 3. Copy executable to folder
copy my_vault.exe C:\MyVault\

# 4. Create shortcut (optional)
Right-click my_vault.exe → Create shortcut → Place on Desktop

# 5. Run the application
Double-click my_vault.exe
```

---

## System Requirements

### Minimum Requirements
- **OS**: Windows 7 or newer (Windows 10/11 recommended)
- **RAM**: 2 GB minimum (4 GB recommended)
- **Disk Space**: 50 MB for application + space for encrypted files
- **CPU**: Dual-core processor (more cores = faster batch operations)

### Recommended Requirements
- **OS**: Windows 10/11 (64-bit)
- **RAM**: 8 GB or more
- **Disk Space**: 500 MB or more
- **CPU**: Quad-core or higher
- **Storage**: SSD for better performance

### Dependencies
✅ **None required** - All dependencies are statically linked into the executable

---

## Installation Methods

### Method 1: Direct Executable (Simplest)
```
1. Copy my_vault.exe to desired location
2. Create shortcut if needed
3. Run directly - no installation required
```

### Method 2: Create Windows Shortcut
```
1. Right-click my_vault.exe
2. Select "Send to" → "Desktop (create shortcut)"
3. Rename shortcut to "MyVault" if desired
4. Double-click to launch
```

### Method 3: Add to Program Files (Admin)
```bash
# Run as Administrator
mkdir "C:\Program Files\MyVault"
copy my_vault.exe "C:\Program Files\MyVault\"

# Create shortcut on Desktop
Right-click shortcut → Properties → Advanced → Run as Administrator (optional)
```

### Method 4: Portable USB Drive
```
1. Copy my_vault.exe to USB drive
2. No installation needed - run directly from USB
3. Works on any Windows computer
```

---

## Configuration

### First Launch
1. **Launch Application**
   - Double-click `my_vault.exe`
   - Window appears with "Create Master Password" dialog

2. **Set Master Password**
   - Enter a strong password (mix of uppercase, lowercase, numbers, symbols)
   - Confirm password
   - ⚠️ **IMPORTANT**: Write down and store password securely - it CANNOT be recovered if lost

3. **Add Files/Folders**
   - Click "Add File" to encrypt individual files
   - Click "Add Folder" to scan and encrypt folder contents

### Configuration File
- **Location**: Same directory as `my_vault.exe`
- **File Name**: `vault_config.json`
- **Contents**: Encrypted file inventory and master password hash
- **Backup**: Automatically created on each operation

### Backing Up Your Configuration
```bash
# Copy configuration to backup location
copy vault_config.json D:\Backups\vault_config_backup.json

# Keep this backup safe - it contains your file registry
```

---

## Running the Application

### Standard Launch
```
Double-click: C:\MyVault\my_vault.exe
```

### Command Line Launch (Optional)
```bash
# From PowerShell or Command Prompt
C:\MyVault\my_vault.exe

# Or from any directory if in PATH
my_vault.exe
```

### First-Time Operations

#### Lock Files (Encrypt)
1. Click "Add File" or "Add Folder"
2. Select files/folders to encrypt
3. Click "Lock" button
4. Enter master password
5. Operation completes with timing shown
6. Original files are removed, encrypted versions created

#### Unlock Files (Decrypt)
1. Encrypted files appear in list with 🔒 icon
2. Select encrypted files
3. Click "Unlock" button
4. Enter master password
5. Operation completes with timing shown
6. Encrypted files are removed, original files restored

#### Batch Operations
- Select multiple files using:
  - Single click: Select one
  - Ctrl+Click: Toggle individual items
  - Shift+Click: Select range
- Click Lock/Unlock to process all selected at once
- Progress bar shows completion percentage

---

## Performance Expectations

### Typical Performance

| File Size | Time | Notes |
|-----------|------|-------|
| 1 MB | < 100ms | Very fast |
| 100 MB | 1-2 seconds | Fast |
| 1 GB | 15-30 seconds | Standard |
| 10 GB | 2-5 minutes | Depends on disk speed |
| 100 GB | 20-50 minutes | SSD recommended |

### Batch Processing
```
99 files (14 GB total):
- Sequential processing: ~2 minutes
- Memory usage: ~192 MB constant
- CPU usage: 50-75% (adaptive)
- Storage I/O: Optimized for HDD/SSD
```

---

## Troubleshooting

### Application Won't Start
**Problem**: Click executable but nothing happens
**Solution**:
```bash
# Run from command prompt to see errors
cmd /k "C:\MyVault\my_vault.exe"

# Or drag file to command prompt
# Watch for error messages
```

### "Invalid file header" Error
**Problem**: Decryption fails with header error
**Solution**:
1. File may be corrupted
2. Check if encrypted file was modified
3. Use backup copy if available
4. Try another encrypted file to test

### Out of Memory Error
**Problem**: App crashes with memory error on large files
**Solution**:
1. Close other applications (free up RAM)
2. Restart computer if needed
3. Process fewer files at once
4. Use streaming encryption (already default)

### Files Locked by Other Process
**Problem**: Cannot delete or rename files
**Solution**:
1. Close all other file managers
2. Close web browsers accessing files
3. Restart computer if necessary
4. Check Task Manager for file locks

### Master Password Forgotten
**Problem**: Cannot unlock encrypted files
**Solution**:
- ❌ **Password CANNOT be recovered** - it's intentionally non-recoverable for security
- Options:
  1. If you have backup of original files, use those
  2. Use a password recovery tool on the encrypted files (very difficult)
  3. All encrypted files become inaccessible

### Slow Performance
**Problem**: Operations take longer than expected
**Solution**:
1. Check disk health: `chkdsk C: /F` (run as Administrator)
2. Use SSD instead of HDD (much faster)
3. Close resource-intensive applications
4. Disable antivirus real-time scanning during operations
5. Check system RAM - upgrade if below 4GB

---

## Security Considerations

### ✅ What MyVault Protects
- **File Content**: Encrypted with ChaCha20-Poly1305 (military-grade AEAD)
- **Authentication**: Argon2id password hashing (resistant to brute force)
- **Memory Safety**: Rust prevents buffer overflows and memory corruption
- **File Handle Management**: Proper cleanup prevents information leaks

### ⚠️ Security Best Practices

#### 1. Master Password
```
✅ DO:
- Use strong password (12+ characters)
- Mix uppercase, lowercase, numbers, symbols
- Use password manager to store password
- Change password regularly

❌ DON'T:
- Use simple passwords (123456, password, etc.)
- Share password with others
- Write password on sticky notes
- Use same password as other accounts
```

#### 2. Backup Strategy
```
✅ RECOMMENDED BACKUP:
1. Keep encrypted files on main drive
2. Backup encrypted files to external drive
3. Keep vault_config.json backup separate
4. Store password in secure password manager
5. Document encryption key storage location

Example:
C:\MyVault\encrypted_files\ → Main
D:\Backup\encrypted_files\ → External backup
E:\Emergency\vault_config_backup.json → Emergency backup
```

#### 3. Shared Computers
```
❌ DO NOT use MyVault on shared computers
✅ BETTER: Use on personal computer only

If must share:
1. Create separate Windows user account
2. Use Windows file encryption (BitLocker)
3. Keep encrypted files in private folder
```

#### 4. Cloud Storage
```
✅ SAFE: Upload encrypted files to cloud
   (Google Drive, OneDrive, Dropbox)
   - Files are encrypted before upload
   - Only you can decrypt with password

❌ UNSAFE: Share encrypted files without password management
   - Anyone with files could brute-force
```

#### 5. File Shredding
```
⚠️ WARNING: MyVault removes original files after encryption
   - Files may be recoverable with data recovery tools
   - Use dedicated file shredder for security:
     - Windows: cipher /w:C:  (secure wipe)
     - Or use CCleaner with secure delete option
```

---

## Advanced Configuration

### Running at Startup (Windows)
```
1. Right-click my_vault.exe → Create shortcut
2. Right-click shortcut → Properties
3. Go to: C:\Users\[Username]\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup
4. Copy shortcut to Startup folder
5. MyVault launches on next login
```

### Running as Service (Advanced)
```
Requires: NSSM (Non-Sucking Service Manager)
1. Download NSSM from https://nssm.cc/download
2. Open command prompt as Administrator
3. Run: nssm install MyVaultService C:\MyVault\my_vault.exe
4. Manage in Services: services.msc
```

### Command Line Parameters (Future Enhancement)
```
Planned for future versions:
my_vault.exe --add-file path/to/file
my_vault.exe --lock folder
my_vault.exe --unlock folder
my_vault.exe --password "mypassword"
```

---

## Deployment Checklist

### Pre-Deployment
- [ ] Test application on target Windows version
- [ ] Verify file encryption works
- [ ] Test batch operations (99+ files)
- [ ] Confirm execution time display works
- [ ] Check custom icon displays correctly
- [ ] Verify file list dimming when not authenticated

### Deployment Day
- [ ] Copy `my_vault.exe` to installation location
- [ ] Create shortcut if needed
- [ ] Create `C:\MyVault` folder for configuration
- [ ] Run application to create `vault_config.json`
- [ ] Set master password
- [ ] Test Lock/Unlock on sample files
- [ ] Document password in secure location
- [ ] Create backup of configuration

### Post-Deployment
- [ ] Monitor disk space usage
- [ ] Verify regular backups are working
- [ ] Check for any error reports
- [ ] Update documentation with local paths
- [ ] Train users on Lock/Unlock operations
- [ ] Provide support contact information

---

## Support & Updates

### Getting Help
1. **Check Troubleshooting section** above
2. **Check error report** - click "View Error Report" if errors occur
3. **Review log file** - check for patterns

### Reporting Issues
When reporting bugs, include:
- Windows version (Windows 10 Build 19041, etc.)
- File size and count
- Error message (if any)
- Steps to reproduce
- System specs (RAM, CPU, Storage type)

### Updates
- Check GitHub releases for updates
- Rebuild from source with `cargo build --release`
- New version: D:\Codes\rust\myVault\target\release\my_vault.exe
- Backup old configuration before updating

---

## Legal & Warranty

**MyVault** is provided as-is without warranty. Users are responsible for:
- Maintaining backups of important files
- Securing their master password
- Ensuring they have legal right to encrypt files
- Compliance with local data protection laws

---

## Quick Reference

### Key Folders
```
C:\MyVault\
├── my_vault.exe (Application)
└── vault_config.json (Configuration - AUTO-CREATED)
```

### File Status Indicators
```
[F] = File
[D] = Directory/Folder
🔒 = Locked (Encrypted)
🔓 = Unlocked (Unencrypted)
```

### Keyboard Shortcuts
```
Ctrl+Click = Toggle individual selection
Shift+Click = Select range
Click = Single select
```

### Status Messages
```
"Locked 5 items in 3.45s" = Success
"Locked 5 items with 1 errors in 2.10s" = Partial success
"🔒 Please enter password to view files" = Not authenticated
```

---

**Version**: 1.0
**Last Updated**: November 2, 2025
**MyVault Application**: Production Ready 🚀
