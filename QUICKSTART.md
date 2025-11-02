# MyVault Quick Start Guide

## 60-Second Setup

### Step 1: Get the Executable
```
Location: D:\Codes\rust\myVault\target\release\my_vault.exe
```

### Step 2: Run It
```
Double-click: my_vault.exe
```

### Step 3: Create Master Password
- App opens with "Create Master Password" dialog
- Enter strong password (e.g., `MySecure!Pass123`)
- Confirm password
- ✅ Password saved

### Step 4: Add Files
- Click "Add File" → Select file(s)
- OR Click "Add Folder" → Select folder

### Step 5: Encrypt Files
- Select file(s) in list
- Click "Lock" button
- Original files become encrypted versions
- ✅ Files encrypted!

### Step 6: Decrypt Files
- Select encrypted file (shows 🔒)
- Click "Unlock" button
- Enter master password
- Encrypted file becomes original
- ✅ Files decrypted!

---

## Essential Operations

### Encrypt a Single File
```
1. Click "Add File"
2. Select file
3. Click "Lock"
4. Done!
```

### Encrypt a Whole Folder
```
1. Click "Add Folder"
2. Select folder
3. Click "Lock"
4. All files encrypted
5. Done!
```

### Decrypt Multiple Files
```
1. Select file 1 (click)
2. Ctrl+Click file 2, 3, 4, ... (add to selection)
3. Click "Unlock"
4. All selected files decrypted
5. Done!
```

### Decrypt Range of Files
```
1. Click file 5
2. Shift+Click file 15
3. All files 5-15 selected
4. Click "Unlock"
5. Done!
```

---

## What You See

### Not Authenticated
```
┌─────────────────────────────┐
│ My Vault App   [Password]   │
├─────────────────────────────┤
│ [Dimmed/Greyed Out]         │
│ 🔒 Please enter password    │
│ to view files               │
│                             │
│ [Buttons disabled]          │
└─────────────────────────────┘
```

### After Password Entered
```
┌─────────────────────────────┐
│ My Vault App   [Password]   │
├─────────────────────────────┤
│ [Add File] [Add Folder]     │
│ [Lock] [Unlock] [Remove]    │
│                             │
│ [D] C:\test  Unlocked  🔓   │
│ [F] file.txt Locked    🔒   │
│ [F] photo.jpg Locked   🔒   │
│                             │
│ Selected: 1                 │
│                             │
│ Unlocked 2 items in 2.34s   │
└─────────────────────────────┘
```

---

## Important Notes

### ⚠️ Password Security
```
✅ REMEMBER: Password cannot be recovered if forgotten
✅ STORE: Write it down somewhere safe
✅ BACKUP: Keep backup of encrypted files
```

### ⏱️ Performance
```
Small files (< 100 MB):  < 1 second
Medium files (1 GB):     15-30 seconds
Large files (10 GB):     2-5 minutes
Batch (99 files):        ~2 minutes
```

### 💾 Your Data
```
Original File → my_vault.exe → Encrypted File
[removed]      [ChaCha20-    [stays on disk]
               Poly1305]

To get back original:
Encrypted File → my_vault.exe → Original File
[stays]        [Decrypt]      [recreated]
```

---

## Troubleshooting

### Black screen/won't start?
```
Right-click executable → Run as Administrator
```

### Password wrong?
```
❌ Cannot recover - password is write-only
✅ Try password again
✅ If forgotten, encrypted files are inaccessible
```

### Files locked by other program?
```
Close file manager and other programs
Restart if necessary
Try again
```

### Out of memory on large folder?
```
✅ Already optimized - shouldn't happen
❌ If it does, close other programs and retry
```

---

## File Icons Explained

| Icon | Meaning |
|------|---------|
| `[F]` | File |
| `[D]` | Folder/Directory |
| `🔒` | Locked (Encrypted) |
| `🔓` | Unlocked (Not encrypted) |

---

## Selection Methods

### Single Select
```
Click one file
→ Only that file selected
```

### Add/Remove Individual
```
Ctrl+Click file
→ Toggle on/off selection
```

### Select Range
```
Click file 1
Shift+Click file 10
→ Files 1-10 all selected
```

### Select All
```
Ctrl+A (when list focused)
→ All files selected
```

---

## Status Messages

```
"Locked 5 items in 3.45s"
→ Success! 5 files encrypted in 3.45 seconds

"Unlocked 85 items with 14 errors in 1m 22.5s"
→ 85 files decrypted, but 14 had issues
→ Click "View Error Report" to see details

"🔒 Please enter password to view files"
→ Password not entered yet
→ Click [Master Password] to enter it
```

---

## Common Workflows

### Workflow 1: Encrypt Laptop Before Travel
```
1. Open MyVault
2. Click "Add Folder" → Select Documents
3. Click "Lock"
4. All files encrypted
5. Safe to travel with laptop
```

### Workflow 2: Secure Sensitive Files
```
1. Open MyVault
2. Click "Add File" → Select sensitive files
3. Shift+Click to select multiple
4. Click "Lock"
5. Files encrypted and hidden
```

### Workflow 3: Share Encrypted Files
```
1. Encrypt files with MyVault
2. Upload encrypted files to cloud
3. Share cloud link with others
4. Only you can decrypt (password protected)
5. Very secure file sharing
```

### Workflow 4: Backup Before Encryption
```
1. Copy original files to backup location
2. Open MyVault
3. Encrypt files on main drive
4. Keep backup encrypted too
5. Two encrypted backups = extra safe
```

---

## Next Steps

1. **Download**: Copy `my_vault.exe` to your computer
2. **Create Folder**: Make `C:\MyVault` folder
3. **Run**: Double-click `my_vault.exe`
4. **Set Password**: Choose strong password
5. **Test**: Encrypt a sample file
6. **Add Files**: Start using MyVault
7. **Backup**: Regular backups of encrypted files
8. **Enjoy**: Your files are now secure! 🎉

---

## Need Help?

See **DEPLOYMENT_GUIDE.md** for detailed documentation:
- System requirements
- Advanced configuration
- Troubleshooting
- Security best practices
- Backup strategies

---

**MyVault v1.0** - Production Ready 🚀
Encrypt. Secure. Simple.
