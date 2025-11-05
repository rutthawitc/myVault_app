# MyVault Testing Guide

## Overview

This guide provides comprehensive instructions for testing all features of myVault, including the new Phase 1 and Phase 2 improvements.

---

## Prerequisites

### Building the Application

**Step 1: Ensure you have Rust installed**
```bash
# Check Rust version (requires 1.70+)
rustc --version

# If not installed, get it from: https://rustup.rs/
```

**Step 2: Build the application**
```bash
# Navigate to project directory
cd /home/user/myVault_app

# Build release version (optimized)
cargo build --release

# The executable will be at:
# target/release/my_vault.exe (Windows)
# target/release/my_vault (Linux/macOS)
```

**Step 3: Run the application**
```bash
# From project root
./target/release/my_vault

# Or on Windows
target\release\my_vault.exe
```

**If build fails:**
- Check internet connection (needs to download dependencies)
- Verify Rust toolchain: `rustup update`
- Clear build cache: `cargo clean` then rebuild
- Check Cargo.toml for syntax errors

---

## Testing Strategy

### Testing Levels

1. **Smoke Test** - Quick verification that app launches and basic functions work
2. **Feature Test** - Detailed testing of each feature
3. **Integration Test** - Test features working together
4. **Stress Test** - Test with large files and many operations

---

## Smoke Test (5 minutes)

Quick verification that the app is working:

### ✅ Checklist

- [ ] App launches without errors
- [ ] Window displays correctly
- [ ] Create master password
- [ ] Add a single test file
- [ ] Lock the test file
- [ ] Unlock the test file
- [ ] Remove file from list
- [ ] Close app and reopen (verify config saved)

**If smoke test fails, do NOT proceed. Fix issues first.**

---

## Phase 1 Feature Testing

### Feature 1: Clipboard Support for Error Reports

**Purpose:** Copy error details to clipboard for troubleshooting

**Test Steps:**

1. **Setup:**
   - Create 3 test files: `test1.txt`, `test2.txt`, `test3.txt`
   - Add all three to myVault
   - Lock them successfully
   - Manually delete `test2.txt.vault.encrypted` (to create error)

2. **Test Execution:**
   - Select all three files
   - Click "Unlock"
   - Operation will fail for test2 (file not found)

3. **Verification:**
   - Click "View Error Report" button (should appear)
   - Verify error report window shows test2 with error message
   - Click "Copy to Clipboard" button
   - Paste into text editor (Ctrl+V)
   - Verify clipboard contains formatted error report:
     ```
     1. C:\path\to\test2.txt.vault.encrypted
        Error: File not found
     ```

**Expected Results:**
- ✅ Error report window displays
- ✅ Copy button works without errors
- ✅ Clipboard contains properly formatted text
- ✅ Status message confirms copy: "Error report copied to clipboard (1 items)"

**Error Cases:**
- ⚠️ "Failed to access clipboard" - OS permission issue
- ⚠️ "Failed to copy to clipboard" - Windows clipboard API error

---

### Feature 2: Password Strength Meter

**Purpose:** Visual feedback on password quality

**Test Steps:**

1. **Test Weak Passwords:**
   - Click "Create Master Password"
   - Type: `password`
   - **Expected:** Red bar (1/3 filled), label "Weak"
   - Type: `12345678`
   - **Expected:** Red bar, label "Weak" (sequential pattern)
   - Type: `aaaaaaaa`
   - **Expected:** Red bar, label "Weak" (repetitive)

2. **Test Medium Passwords:**
   - Type: `Password123`
   - **Expected:** Yellow bar (2/3 filled), label "Medium"
   - Type: `mypassword1`
   - **Expected:** Yellow bar, label "Medium"

3. **Test Strong Passwords:**
   - Type: `MyP@ssw0rd2024!`
   - **Expected:** Green bar (3/3 filled), label "Strong"
   - Type: `Tr0ub4dor&3`
   - **Expected:** Green bar, label "Strong"

4. **Test Real-time Updates:**
   - Start typing: `a` → Should show immediately
   - Continue typing: `ab` → Updates instantly
   - Clear and retype → Bar should reset

5. **Test in Change Password Dialog:**
   - Create password and authenticate
   - Click "Change Password"
   - Enter current password
   - Test new password field → Strength meter should appear

**Expected Results:**
- ✅ Strength bar appears below password field
- ✅ Bar fills proportionally (33%, 67%, 100%)
- ✅ Color matches strength (red/yellow/green)
- ✅ Label shows correct text (Weak/Medium/Strong)
- ✅ Updates in real-time as you type
- ✅ Works in both creation and change dialogs

**Pattern Detection:**
- ✅ Sequential: `abcd`, `1234`, `dcba`, `4321` → Weak
- ✅ Repetitive: `aaa`, `111`, `xxx` → Weak
- ✅ All lowercase: `password` → Weak
- ✅ Good mix: `MyP@ss123` → Strong

---

### Feature 3: Real-time Throughput/ETA Display

**Purpose:** Show processing speed and estimated completion time

**Test Steps:**

1. **Preparation:**
   - Create 20 test files of varying sizes:
     - 10 small files (1-10 KB)
     - 5 medium files (1-10 MB)
     - 5 large files (50-100 MB)
   - Add all to myVault

2. **Test Encryption:**
   - Select all 20 files
   - Click "Lock"
   - **Observe progress window:**
     - Main text: "Processed X of 20 (0 errors)"
     - Blue text: "Speed: X.X files/s  ETA: Xs"
   - Progress bar should fill gradually

3. **Verify Throughput Display:**
   - **Small files:** Speed should be high (5-10+ files/s)
   - **Large files:** Speed should drop (0.1-2 files/s)
   - **Format:** "Speed: 2.5 files/s"

4. **Verify ETA Display:**
   - **< 60 seconds:** "ETA: 45s"
   - **1-60 minutes:** "ETA: 2.5m"
   - **> 60 minutes:** "ETA: 1.2h"

5. **Test Decryption:**
   - Select all 20 encrypted files
   - Click "Unlock"
   - Verify throughput and ETA appear

**Expected Results:**
- ✅ Throughput displays in blue text
- ✅ Format: "Speed: X.X files/s"
- ✅ ETA format changes based on time remaining
- ✅ Updates in real-time as files process
- ✅ Both metrics disappear during scanning phase
- ✅ Metrics stay visible until operation completes

**Edge Cases:**
- Very fast operation (1 file, 1 KB) → May show 0.0 files/s briefly
- Very slow operation (1 file, 10 GB) → ETA in hours

---

### Feature 4: Dark Mode Toggle

**Purpose:** Switch between light and dark themes

**Test Steps:**

1. **Initial State:**
   - Launch app (should be in light mode by default)
   - **Verify:** Light background, dark text

2. **Toggle to Dark Mode:**
   - Click "🌙 Dark Mode" button in top panel
   - **Expected:**
     - Background becomes dark
     - Text becomes light
     - Button changes to "☀ Light Mode"
     - All dialogs use dark theme

3. **Test All Dialogs in Dark Mode:**
   - Password creation dialog
   - Password change dialog
   - Confirmation dialogs (Lock/Unlock/Remove)
   - Error report window
   - Progress window
   - **Verify:** All readable with proper contrast

4. **Toggle Back to Light Mode:**
   - Click "☀ Light Mode" button
   - **Expected:** Returns to light theme
   - Button changes back to "🌙 Dark Mode"

5. **Test Restart Persistence:**
   - Toggle to dark mode
   - Close app
   - Reopen app
   - **Expected:** Should remember dark mode setting (once preferences are saved)

**Expected Results:**
- ✅ Toggle button in top panel
- ✅ Emoji indicates current mode
- ✅ Instant theme switching (no flicker)
- ✅ All UI elements respect theme
- ✅ Text remains readable in both modes
- ✅ Dialogs and windows use correct theme

**Visual Checks:**
- Top panel background changes
- Button backgrounds change
- Text colors invert appropriately
- Scroll bars match theme
- Selection highlights visible

---

## Phase 2 Feature Testing

### Feature 5: Drag and Drop File Support

**Purpose:** Add files by dragging from file explorer

**Test Steps:**

1. **Test Single File Drag:**
   - Open Windows Explorer/Finder
   - Select a file (e.g., `document.txt`)
   - Drag over myVault window
   - Drop into file list area
   - **Expected:** File added to list immediately

2. **Test Multiple Files Drag:**
   - Select 5 files in Explorer
   - Drag all together
   - Drop into myVault
   - **Expected:** All 5 files added

3. **Test Folder Drag:**
   - Drag a folder from Explorer
   - Drop into myVault
   - **Expected:** Folder added as [D] item type

4. **Test Drop Zones:**
   - Try dropping in different areas:
     - ✅ File list scroll area → Should work
     - ❌ Top panel → Should NOT work (expected)
     - ❌ Bottom status bar → Should NOT work (expected)

5. **Test While Busy:**
   - Start a long encryption operation
   - Try dragging files while busy
   - **Expected:** Files might not be added (safety feature)

6. **Test Cross-Platform:**
   - Windows: From Explorer
   - macOS: From Finder
   - Linux: From Nautilus/Dolphin

**Expected Results:**
- ✅ Files appear in list immediately
- ✅ Multiple files work at once
- ✅ Folders detected correctly ([D] marker)
- ✅ No duplicates if file already in list
- ✅ Status message confirms addition
- ✅ Works from any file manager

**Hint Message:**
- When list is empty: "No files added yet. Use buttons above or drag & drop files here."

---

### Feature 6: Search/Filter in File List

**Purpose:** Quickly find files by name

**Test Steps:**

1. **Preparation:**
   - Add 20+ files with different names:
     - documents.txt
     - report_2024.pdf
     - photo.jpg
     - backup.zip
     - etc.

2. **Test Basic Search:**
   - In search box, type: `report`
   - **Expected:** Only files with "report" in name show
   - Clear search (✖ button)
   - **Expected:** All files reappear

3. **Test Case Insensitivity:**
   - Type: `DOCUMENT`
   - **Expected:** Shows "documents.txt" (case insensitive)

4. **Test Partial Matches:**
   - Type: `doc`
   - **Expected:** Shows all files containing "doc"

5. **Test No Matches:**
   - Type: `zzzznonexistent`
   - **Expected:** Message "No items match the search filter"

6. **Test Search with Path:**
   - Type: `C:\Users`
   - **Expected:** Shows all files in that path

7. **Test Clear Button:**
   - Enter search text
   - Click ✖ button
   - **Expected:** Search clears, all files show

8. **Test Search Persistence:**
   - Enter search
   - Select filtered file
   - **Expected:** Selection works correctly
   - Clear search
   - **Expected:** Selection maintained

**Expected Results:**
- ✅ Filters in real-time as you type
- ✅ Case insensitive
- ✅ Matches anywhere in path
- ✅ Clear button (✖) works
- ✅ Shows helpful message when no matches
- ✅ Selection state preserved
- ✅ Search box has hint text

---

### Feature 7: File Size Display

**Purpose:** Show human-readable file sizes

**Test Steps:**

1. **Create Test Files:**
   - Small: 500 bytes → Should show "500 B"
   - Medium: 2.5 KB → Should show "2.5 KB"
   - Large: 15.3 MB → Should show "15.3 MB"
   - Huge: 2.1 GB → Should show "2.10 GB"

2. **Add Files to myVault:**
   - Add all test files
   - **Verify format:**
     ```
     [F]  small.txt  500 B  Unlocked 🔓
     [F]  medium.pdf  2.5 KB  Unlocked 🔓
     [F]  large.zip  15.3 MB  Unlocked 🔓
     [F]  huge.iso  2.10 GB  Unlocked 🔓
     ```

3. **Test Folders:**
   - Add a folder
   - **Expected:** Shows "N/A" (folders don't have single size)

4. **Test Missing Files:**
   - Add file, then delete it from disk
   - **Expected:** Shows "N/A"

5. **Test After Encryption:**
   - Lock a file
   - **Expected:** Size still shown (reads from encrypted file)

**Expected Results:**
- ✅ Sizes formatted with proper units (B/KB/MB/GB)
- ✅ One decimal place for KB/MB
- ✅ Two decimal places for GB
- ✅ Falls back to "N/A" gracefully
- ✅ Visible in file list between path and status

---

### Feature 8: Sort Functionality

**Purpose:** Organize files by different criteria

**Test Steps:**

1. **Test Sort by Name:**
   - Add files: zebra.txt, apple.txt, middle.txt
   - Click "Name" sort button
   - **Expected:** apple, middle, zebra (A-Z)
   - Click "Name" again
   - **Expected:** zebra, middle, apple (Z-A)
   - **Verify:** ⬆ or ⬇ arrow shows direction

2. **Test Sort by Status:**
   - Have mix of locked and unlocked files
   - Click "Status" button
   - **Expected:** Groups locked and unlocked together
   - Click again
   - **Expected:** Reverses order

3. **Test Sort by Size:**
   - Have files of different sizes
   - Click "Size" button
   - **Expected:** Smallest to largest
   - Click again
   - **Expected:** Largest to smallest

4. **Test Sort Persistence:**
   - Set sort to "Size" descending
   - Add new file
   - **Expected:** New file inserted in sorted position
   - Perform lock/unlock
   - **Expected:** Sort order maintained

5. **Test Visual Indicators:**
   - **Selected button:** Should be highlighted
   - **Arrow:** ⬆ for ascending, ⬇ for descending
   - **Unselected buttons:** Normal appearance

**Expected Results:**
- ✅ Three sort buttons: Name, Status, Size
- ✅ Click toggles ascending/descending
- ✅ Selected button highlighted
- ✅ Arrow shows direction
- ✅ Sort applies immediately
- ✅ New files inserted correctly
- ✅ Tooltips on hover explain each sort

**Sort Tooltips:**
- Name: "Sort by filename"
- Status: "Sort by lock status"
- Size: "Sort by file size"

---

### Feature 9: Keyboard Shortcuts

**Purpose:** Fast operations without mouse

**Test Steps:**

1. **Test Ctrl+A (Select All):**
   - Add 10 files
   - Press Ctrl+A
   - **Expected:** All 10 files selected
   - Count shows "Selected: 10"

2. **Test Ctrl+L (Lock):**
   - Select unlocked files
   - Press Ctrl+L
   - **Expected:** Lock confirmation dialog appears
   - Cancel → Dialog closes
   - Try again → Confirm → Files encrypt

3. **Test Ctrl+U (Unlock):**
   - Select locked files
   - Press Ctrl+U
   - **Expected:** Unlock confirmation dialog appears

4. **Test Delete Key (Remove):**
   - Select 2 files
   - Press Delete
   - **Expected:** Remove confirmation dialog
   - Confirm → Files removed from list (not deleted from disk)

5. **Test Escape (Clear Selection):**
   - Select multiple files
   - Press Escape
   - **Expected:** All files deselected
   - Count shows nothing

6. **Test Shortcuts When Disabled:**
   - Open password dialog
   - Try Ctrl+A
   - **Expected:** No effect (shortcuts disabled in dialogs)
   - Close dialog → Shortcuts work again

7. **Test While Busy:**
   - Start batch operation
   - Try shortcuts
   - **Expected:** No effect (safety feature)

**Expected Results:**
- ✅ All shortcuts work as described
- ✅ Shortcuts disabled in dialogs
- ✅ Shortcuts disabled when busy
- ✅ Visual feedback for each action
- ✅ No conflicts with OS shortcuts

**Shortcut Reference:**
| Shortcut | Action |
|----------|--------|
| Ctrl+A | Select All |
| Ctrl+L | Lock Selected |
| Ctrl+U | Unlock Selected |
| Delete | Remove Selected |
| Escape | Clear Selection |

---

### Feature 10: Tooltips

**Purpose:** Helpful descriptions on hover

**Test Steps:**

1. **Test Button Tooltips:**
   - Hover over "Add File"
   - **Expected:** "Add a single file to encrypt/decrypt"
   - Hover over "Lock"
   - **Expected:** "Encrypt selected files (Ctrl+L)"
   - Test all major buttons

2. **Test Sort Tooltips:**
   - Hover over "Name" button
   - **Expected:** "Sort by filename"
   - Test Status and Size buttons

3. **Test Clear Search Tooltip:**
   - Hover over ✖ button
   - **Expected:** "Clear search"

4. **Test Tooltip Timing:**
   - Tooltips should appear after ~0.5 second hover
   - Should disappear when mouse moves away
   - Should not block interaction

5. **Test in Dark Mode:**
   - Switch to dark mode
   - Verify tooltips are readable

**Expected Results:**
- ✅ All major buttons have tooltips
- ✅ Keyboard shortcuts mentioned in tooltips
- ✅ Tooltips appear on hover
- ✅ Tooltips disappear on mouse leave
- ✅ Readable in both light and dark mode
- ✅ Don't obstruct clicking

**Buttons with Tooltips:**
- Add File
- Add Folder
- Scan for Locked Files
- Lock
- Unlock
- Remove
- Name sort
- Status sort
- Size sort
- Clear search

---

## Integration Testing

Test features working together:

### Test 1: Complete Workflow

1. Start app in dark mode
2. Drag 10 files into app
3. Use search to filter to 5 files
4. Select all filtered (Ctrl+A)
5. Sort by size
6. Lock selected (Ctrl+L)
7. Clear search
8. Select locked files
9. Unlock (Ctrl+U)
10. Remove all (Delete)

**Verify:** All features work smoothly together

### Test 2: Error Handling

1. Add 3 files
2. Lock them
3. Manually corrupt one encrypted file (edit with text editor)
4. Try to unlock all
5. View error report
6. Copy errors to clipboard
7. Verify clipboard content

### Test 3: Session Flow

1. Create password with strength meter (use strong password)
2. Add files via drag & drop
3. Sort by name
4. Mark some as favorite (if implemented)
5. Close app
6. Reopen
7. Verify preferences saved (dark mode, sort)
8. Verify files still in list

---

## Stress Testing

### Large File Test

**Purpose:** Verify streaming encryption works

**Test:**
1. Create 5 GB test file
2. Add to myVault
3. Lock it
4. Monitor: Memory usage should stay ~48MB
5. Verify progress shows throughput/ETA
6. Unlock it
7. Compare checksums (original vs decrypted)

**Expected:** Constant memory usage, successful operation

### Many Files Test

**Purpose:** Test parallel processing

**Test:**
1. Add 100 files (various sizes)
2. Lock all at once
3. Verify: Up to 4 parallel operations
4. Monitor progress window
5. Check for memory leaks
6. Verify all files encrypted successfully

**Expected:** Stable performance, no crashes

### Long Session Test

**Purpose:** Test stability over time

**Test:**
1. Run app for 2+ hours
2. Perform various operations
3. Lock/unlock multiple times
4. Search, sort, select
5. Monitor memory usage
6. Check for memory leaks

**Expected:** Stable memory, no slowdowns

---

## Bug Reporting Template

If you find issues, report with this information:

```markdown
**Bug Title:** [Brief description]

**Steps to Reproduce:**
1. Step one
2. Step two
3. Step three

**Expected Behavior:**
What should happen

**Actual Behavior:**
What actually happens

**Screenshots:**
[If applicable]

**Environment:**
- OS: Windows 10 / Linux / macOS
- App Version: v1.0.0 + Phase 1 & 2
- Files tested: [Number and sizes]

**Additional Context:**
Any other relevant information
```

---

## Testing Checklist Summary

### Phase 1 Features
- [ ] Clipboard support works
- [ ] Password strength meter accurate
- [ ] Throughput/ETA displays correctly
- [ ] Dark mode toggle functional

### Phase 2 Features
- [ ] Drag and drop works
- [ ] Search filters correctly
- [ ] File sizes display properly
- [ ] Sort functionality works
- [ ] Keyboard shortcuts functional
- [ ] Tooltips appear on hover

### Integration
- [ ] Features work together
- [ ] No conflicts or crashes
- [ ] Preferences save/load

### Performance
- [ ] Large files (> 1 GB) work
- [ ] Many files (100+) work
- [ ] Memory stays constant
- [ ] No memory leaks

---

## Quick Test Script

For rapid testing, use this sequence:

```bash
# 1. Build
cargo build --release

# 2. Run
./target/release/my_vault

# 3. Quick test sequence:
- Create password (test strength meter)
- Toggle dark mode
- Drag 5 files
- Type "test" in search
- Sort by size
- Ctrl+A (select all)
- Ctrl+L (lock - confirm)
- Wait for completion (watch throughput)
- Ctrl+A again
- Ctrl+U (unlock - confirm)
- Delete (remove - confirm)

# 4. Close and reopen
- Verify config saved
```

**Expected time:** 2-3 minutes for quick test

---

## Troubleshooting

### Build Issues

**Problem:** `cargo build` fails
**Solution:**
```bash
# Check Rust version
rustc --version

# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Runtime Issues

**Problem:** App crashes on start
**Solution:**
- Check config file: `~/.local/share/myvault/vault_config.json`
- Delete config to reset
- Check file permissions

**Problem:** Clipboard doesn't work
**Solution:**
- Windows: Check Windows clipboard service
- Linux: Install xclip or xsel
- macOS: Should work natively

**Problem:** Drag & drop doesn't work
**Solution:**
- Check OS permissions
- Try running as administrator
- Verify files aren't locked by another program

---

## Success Criteria

Your testing is successful if:

✅ All Phase 1 features work as described
✅ All Phase 2 features work as described
✅ No crashes during normal use
✅ Files encrypt/decrypt correctly
✅ Memory usage stays reasonable
✅ App feels responsive
✅ Dark mode is readable
✅ No data loss

---

## Next Steps After Testing

Once testing is complete:

1. **Document any bugs** found
2. **Note performance** observations
3. **Collect user feedback** (if applicable)
4. **Decide on** Phase 3 implementation
5. **Celebrate** successful testing! 🎉

---

## Contact & Support

If you find critical issues:
- Open GitHub issue
- Include testing checklist results
- Attach logs if available
- Describe reproduction steps

**Happy Testing!** 🧪
