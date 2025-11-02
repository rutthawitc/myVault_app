# MyVault Windows Installer Guide

## Option 1: Simple Portable (No Installation)

### Easiest Method
```
1. Copy: my_vault.exe → C:\MyVault\
2. Create shortcut on Desktop
3. Done! No installation needed
```

**Pros**:
- ✅ No admin rights needed
- ✅ No registry changes
- ✅ Can run from USB drive
- ✅ Easy uninstall (just delete folder)

**Cons**:
- ❌ No Start Menu entry
- ❌ Manual shortcut creation

---

## Option 2: Program Files Installation

### With Admin Rights
```bash
# Run Command Prompt as Administrator

mkdir "C:\Program Files\MyVault"
copy "D:\Codes\rust\myVault\target\release\my_vault.exe" "C:\Program Files\MyVault\"
mkdir "C:\Users\%USERNAME%\AppData\Local\MyVault"
```

### Create Start Menu Shortcut
```bash
# PowerShell as Administrator

$TargetPath = "C:\Program Files\MyVault\my_vault.exe"
$ShortcutPath = "C:\ProgramData\Microsoft\Windows\Start Menu\Programs\MyVault.lnk"

$Shell = New-Object -COM WScript.Shell
$Shortcut = $Shell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $TargetPath
$Shortcut.Save()
```

### Create Desktop Shortcut
```bash
# PowerShell as Administrator

$TargetPath = "C:\Program Files\MyVault\my_vault.exe"
$ShortcutPath = "C:\Users\%USERNAME%\Desktop\MyVault.lnk"

$Shell = New-Object -COM WScript.Shell
$Shortcut = $Shell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $TargetPath
$Shortcut.Save()
```

**Pros**:
- ✅ Professional installation
- ✅ Start Menu entry
- ✅ Desktop shortcut
- ✅ Uninstall via Control Panel (optional)

**Cons**:
- ❌ Requires admin rights
- ❌ Manual cleanup if uninstalling

---

## Option 3: Use NSSM for Service Installation

### Install NSSM (Non-Sucking Service Manager)
```bash
# 1. Download NSSM
# Visit: https://nssm.cc/download
# Download: nssm-2.24-101-g897c7ef.zip

# 2. Extract to Program Files
mkdir "C:\Program Files\NSSM"
# Copy nssm.exe to folder

# 3. Add to PATH (optional)
setx PATH "%PATH%;C:\Program Files\NSSM"
```

### Install MyVault as Service
```bash
# Run Command Prompt as Administrator

cd "C:\Program Files\NSSM"

# Install service
nssm install MyVaultService "C:\Program Files\MyVault\my_vault.exe"

# Start service
nssm start MyVaultService

# Check status
nssm status MyVaultService
```

### Manage Service
```bash
# View installed services
services.msc

# Or command line
sc query MyVaultService

# Stop service
nssm stop MyVaultService

# Restart service
nssm restart MyVaultService

# Remove service
nssm remove MyVaultService confirm
```

**Pros**:
- ✅ Runs at system startup
- ✅ Auto-restart on crash
- ✅ No user login required
- ✅ Professional deployment

**Cons**:
- ❌ Requires NSSM installation
- ❌ More complex setup
- ❌ Not suitable for desktop GUI app (MyVault is GUI)

⚠️ **NOTE**: MyVault is a GUI application - service installation not recommended

---

## Option 4: Advanced MSI Installer (WiX Toolset)

### Prerequisites
```bash
# Install WiX Toolset
# Download: https://github.com/wixtoolset/wix3/releases
# Or: choco install wixtoolset
```

### Create WiX Project Structure
```
MyVault_Installer/
├── Product.wxs
├── License.txt
└── Banner.bmp
```

### Sample Product.wxs
```xml
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product
        Id="*"
        Name="MyVault"
        Language="1033"
        Version="1.0.0.0"
        Manufacturer="MyVault"
        UpgradeCode="PUT-GUID-HERE">

        <Package
            InstallerVersion="200"
            Compressed="yes"
            InstallScope="perMachine"/>

        <Media Id="1" Cabinet="MyVault.cab" EmbedCab="yes"/>

        <Feature Id="ProductFeature" Title="MyVault" Level="1">
            <ComponentRef Id="MainExecutable"/>
        </Feature>

        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder">
                <Directory Id="INSTALLFOLDER" Name="MyVault"/>
            </Directory>
        </Directory>

        <DirectoryRef Id="INSTALLFOLDER">
            <Component Id="MainExecutable" Guid="PUT-GUID-HERE">
                <File Id="MyVaultEXE"
                      Source="my_vault.exe"
                      KeyPath="yes"/>
            </Component>
        </DirectoryRef>
    </Product>
</Wix>
```

### Build MSI
```bash
# Compile WiX source
candle.exe Product.wxs -o obj\

# Link to create MSI
light.exe -out MyVault.msi obj\Product.wixobj
```

**Pros**:
- ✅ Professional .msi installer
- ✅ Add/Remove Programs entry
- ✅ Automatic uninstall
- ✅ Version tracking

**Cons**:
- ❌ Complex to set up
- ❌ Requires WiX Toolset
- ❌ Learning curve
- ❌ Only for Windows

---

## Option 5: Portable USB Installation

### Create Portable MyVault
```bash
# 1. Create USB structure
D:\USB\MyVault\
├── my_vault.exe
├── vault_config.json (auto-created on first run)
└── README.txt

# 2. Copy files to USB
copy my_vault.exe D:\USB\MyVault\

# 3. Create launcher batch file
# Name: RUN_MYVAULT.bat
# Content:
@echo off
cd /d "%~dp0"
my_vault.exe
pause
```

### Usage on Any Computer
```
1. Insert USB drive
2. Navigate to MyVault folder
3. Double-click my_vault.exe (or RUN_MYVAULT.bat)
4. App runs instantly
5. Works on any Windows computer
```

**Pros**:
- ✅ Truly portable
- ✅ No installation required
- ✅ Works on any computer
- ✅ Easy to copy

**Cons**:
- ❌ Slower from USB
- ❌ Limited disk space
- ❌ No Start Menu entry

---

## Recommended Deployment Methods

### For Single User
```
✅ Option 1 (Portable)
- Copy to C:\MyVault\
- Create Desktop shortcut
- Simple and effective
```

### For Multiple Users
```
✅ Option 2 (Program Files)
- Install to C:\Program Files\MyVault\
- Create Start Menu entry
- Professional appearance
```

### For Enterprise/Corporate
```
✅ Option 4 (MSI Installer)
- Deploy via Group Policy
- Version control
- Automatic updates
```

### For Portable/Travel
```
✅ Option 5 (USB Drive)
- Run from USB stick
- No installation needed
- Works everywhere
```

---

## Uninstallation

### Option 1 & 5 (Portable)
```bash
# Simply delete folder
rmdir /s "C:\MyVault\"

# Or right-click folder → Delete
```

### Option 2 (Program Files)
```bash
# Run as Administrator
rmdir /s "C:\Program Files\MyVault\"

# Delete shortcuts
del "C:\Users\%USERNAME%\Desktop\MyVault.lnk"
del "C:\ProgramData\Microsoft\Windows\Start Menu\Programs\MyVault.lnk"
```

### Option 3 (Service)
```bash
# Run as Administrator
nssm remove MyVaultService confirm

# Delete installation
rmdir /s "C:\Program Files\MyVault\"
```

### Option 4 (MSI)
```bash
# Control Panel → Programs → Uninstall a program
# Find "MyVault"
# Click Uninstall
```

---

## Configuration File Location

### For Portable Installation
```
C:\MyVault\vault_config.json
```

### For Program Files Installation
```
C:\Users\<USERNAME>\AppData\Local\MyVault\vault_config.json
```

### For Multiple User Installations
```
Each user has their own:
C:\Users\Alice\AppData\Local\MyVault\vault_config.json
C:\Users\Bob\AppData\Local\MyVault\vault_config.json
```

---

## Backup Before Installation

### Create Backup Folder
```bash
# Create backup location
mkdir C:\MyVault_Backup

# Copy executable (for reference)
copy my_vault.exe C:\MyVault_Backup\

# Create backup script
# Name: backup_vault.bat
@echo off
set BACKUP_DIR=D:\MyVault_Backups\%date:~-4,4%-%date:~-10,2%-%date:~-7,2%
mkdir "%BACKUP_DIR%"
xcopy "C:\MyVault\*" "%BACKUP_DIR%\" /I /Y
echo Backup created at: %BACKUP_DIR%
pause
```

### Schedule Automatic Backups
```bash
# Windows Task Scheduler
# Create task to run backup_vault.bat daily
# Schedule: Daily at 11:59 PM
```

---

## Installation Verification

### Checklist After Installation
- [ ] Executable runs without errors
- [ ] Custom lock icon displays
- [ ] No black console window visible
- [ ] Master password dialog appears
- [ ] Can create master password
- [ ] Can add files
- [ ] Can encrypt files (Lock)
- [ ] Can decrypt files (Unlock)
- [ ] Execution time displays correctly
- [ ] File list dims when not authenticated
- [ ] Shift+Click range selection works
- [ ] Error reporting works

---

## Troubleshooting Installation

### Issue: "Windows cannot find executable"
**Solution**: Check file path, use quotes for paths with spaces
```bash
"C:\Program Files\MyVault\my_vault.exe"
```

### Issue: "Admin rights required"
**Solution**: Run Command Prompt as Administrator
```bash
Right-click cmd.exe → Run as administrator
```

### Issue: .NET Framework error
**Solution**: MyVault doesn't need .NET - needs Windows 7+

### Issue: Antivirus blocks file
**Solution**: Add to antivirus whitelist
```
C:\Program Files\MyVault\my_vault.exe
```

### Issue: Configuration file not created
**Solution**:
1. Check write permissions on folder
2. Run as Administrator
3. Try different installation path

---

## Distribution Methods

### Method 1: Direct Download
- Host on website or cloud storage
- Users download and run installer
- Simple and direct

### Method 2: Portable Package
- Create .zip with my_vault.exe
- Users extract and run
- No installation needed

### Method 3: GitHub Releases
- Upload .exe to GitHub Releases
- Version tracking automatic
- Easy to share

### Method 4: Package Manager
```bash
# WinGet (Future)
winget install MyVault

# Chocolatey (Future)
choco install myvault
```

---

## Version Updates

### Update Installation
```bash
# 1. Stop MyVault if running
# 2. Backup old executable
copy "C:\Program Files\MyVault\my_vault.exe" "C:\Program Files\MyVault\my_vault.exe.old"

# 3. Copy new executable
copy new_my_vault.exe "C:\Program Files\MyVault\my_vault.exe"

# 4. Test
"C:\Program Files\MyVault\my_vault.exe"

# 5. If old version needed
copy "C:\Program Files\MyVault\my_vault.exe.old" "C:\Program Files\MyVault\my_vault.exe"
```

### Preserve Configuration
```bash
# Configuration file is NOT overwritten
# vault_config.json automatically preserved
# Your encrypted files remain accessible
```

---

**MyVault Installer Guide Complete** ✅

Choose Option 1 (Portable) for simplicity
Choose Option 2 (Program Files) for professionalism
Choose Option 4 (MSI) for enterprise deployment

🚀 Ready to deploy!
