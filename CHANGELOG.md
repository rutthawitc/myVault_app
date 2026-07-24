# Changelog

All notable changes to MyVault are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- **Envelope encryption (DEK/KEK).** Files are now encrypted with a random Data
  Encryption Key that never changes. The master password only wraps that key.
  Changing the master password re-wraps the same key instead of deriving a new
  one — previously this permanently orphaned every already-encrypted file.
  Existing vaults are migrated automatically on the next successful login, and
  files encrypted before the change remain readable.
- **File format V3** with authenticated chunk framing. Each chunk is bound to its
  index and to an end-of-file marker via AEAD additional data, so truncating,
  reordering or splicing chunks is now detected instead of silently producing
  partial plaintext. V1 and V2 files are still readable.
- Chunk lengths read from a file are bounded before allocating, so a corrupted or
  malicious file can no longer trigger a huge allocation.
- The encryption key is wiped from memory when the vault is locked, on session
  timeout, and when a worker thread finishes.

### Fixed

- Encrypted output is flushed **and fsynced** before the plaintext original is
  deleted. A crash or power loss mid-operation can no longer destroy both copies.
- Locking a file no longer overwrites an existing encrypted file with the same
  name; the affected file is reported instead.
- `vault_config.json` is fsynced before the atomic rename.
- A corrupted `vault_config.json` is no longer silently replaced with defaults
  (which discarded the key material). It is backed up and reported instead.
- The UI no longer sleeps 5ms on its own thread for every file queued in a batch
  operation, which made the interface stutter on large batches.

### Changed

- Upgraded eframe/egui from 0.27 to 0.32.
- `config::save_config` takes a single `&Config` instead of a long positional
  argument list, so adding a setting no longer changes its signature.

### Removed

- Deleted unused modules `storage.rs`, `prefetch.rs`, `throughput.rs` and
  `progress.rs` (~1,250 lines) — none were reachable from the application.
  `performance.rs` is trimmed to the CPU/thread detection that is actually used.
- Deleted the unused parallel encryption path (`encrypt_file_parallel` /
  `decrypt_file_parallel`) and the unused `encrypt_blob` / `decrypt_blob` helpers.
  Removing `encrypt_blob` also means the app can no longer write the legacy V1
  format; V1 files remain readable.
- Dropped the now-unused `rayon`, `memmap2` and `generic-array` dependencies and a
  duplicate `winapi` entry.

## [1.0.0] - 2025-11-02

### Added

#### Core Encryption
- ChaCha20-Poly1305 AEAD encryption (military-grade)
- Argon2id password hashing (brute-force resistant)
- Streaming encryption mode (16MB chunks)
- Automatic master nonce generation
- Per-chunk derived nonces (STREAM construction)

#### Batch Operations
- Lock/Unlock multiple files simultaneously
- Support for 99+ files (14GB+ tested)
- Parallel processing (4 concurrent operations)
- Real-time progress tracking with ETA
- Execution time display on completion
- Error reporting with detailed feedback

#### User Interface
- Custom lock icon (256×256 pixels)
- Hidden console window (professional appearance)
- Master password authentication required
- File list with visual lock indicators (🔒/🔓)
- Selection indicators (Selected: X items)

#### Multi-Select Functionality
- Single click: Select one item
- Ctrl+Click: Toggle individual items
- Shift+Click: Select range (bidirectional)
- Last selected tracking
- Range selection from any direction

#### File Management
- Add single files or entire folders
- Scan for locked files in directory
- Remove files from registry
- Automatic file deletion after encryption/decryption
- Proper file handle cleanup

#### Configuration Management
- Auto-generated vault_config.json
- Persistent encrypted file registry
- Master password hash storage
- Automatic backup on each operation

#### Performance Optimization
- Auto-detect CPU core count
- Adaptive thread pool sizing
- Storage type detection (SSD/HDD/Network)
- Chunked I/O (16MB constant memory usage)
- Buffered file operations (32MB buffer)
- File handle cleanup between operations
- Throughput monitoring

#### Security Features
- Memory-safe code (Rust)
- Prevents buffer overflows
- Automatic memory cleanup
- Secure password hashing
- No temporary unencrypted files
- File handle limit protection

#### Authentication & UI
- Master password creation dialog
- Password authentication requirement
- File list dimming when not authenticated
- Message: "🔒 Please enter password to view files"
- Status messages with color coding
- Error report button with details

#### Timing & Analytics
- Operation start/end tracking
- Execution time calculation
- Smart time formatting (ms, seconds, minutes)
- Throughput monitoring
- Status messages: "Locked X items in Y time"

#### Documentation
- Quick start guide (60-second setup)
- Complete deployment guide
- Installer guide (5 installation methods)
- Security documentation
- Architecture documentation
- Contribution guidelines

### Fixed

#### Memory Management
- Fixed memory exhaustion on batch operations
- Reduced chunk size from 64MB to 16MB
- Reduced buffer sizes from 128-256MB to 32MB
- Added explicit file cleanup with drop()
- Proper resource deallocation

#### File Handle Management
- Fixed file descriptor exhaustion
- Added explicit file handle cleanup
- Increased OS yield to 5ms between operations
- Prevented handle accumulation

#### Parallelization
- Disabled problematic parallel encryption (collect in memory)
- Disabled parallel decryption (reads entire file)
- Switched to streaming-only approach
- Capped parallel operations at 4

#### UI/UX
- Console window no longer visible
- Custom icon displays correctly
- File list properly dims when not authenticated
- Status messages display execution time
- Selection state persists correctly

### Security

- Military-grade encryption (ChaCha20-Poly1305)
- Brute-force resistant password hashing (Argon2id)
- Memory-safe implementation (Rust)
- Proper resource cleanup
- No information leaks through file handles
- No unencrypted temporary files
- Secure master password storage (hashed only)

### Performance

#### Benchmarks
- 1 MB file: 100ms
- 100 MB file: 1-2 seconds
- 1 GB file: 15-30 seconds
- 10 GB file: 2-5 minutes
- 99 files (14 GB): ~2 minutes

#### Memory
- Constant memory usage (streaming)
- Per-operation: 48MB max
- Parallel (4 ops): 192MB max
- Scales to any file size

#### Throughput
- HDD optimized
- SSD optimized
- Network path detection
- Adaptive buffering

### Build & Distribution

- Fully portable executable (10MB)
- No dependencies required
- Single .exe file
- Works on Windows 7+
- Release build optimized
- Console window hidden (production)

### Testing

- Unit tests for encryption/decryption
- Batch operation tests
- Memory usage tests
- File handle tests
- UI integration tests
- 41/41 tests passing

---

## [Unreleased]

### Planned for v1.1

- [ ] Linux support
- [ ] macOS support
- [ ] Drag and drop file support
- [ ] Export encryption statistics
- [ ] Custom chunk size options
- [ ] Progress pause/resume
- [ ] Batch preview before execution
- [ ] Keyboard shortcuts

### Planned for v1.2

- [ ] Directory encryption (encrypt whole folders as one)
- [ ] Password strength meter
- [ ] Recent files list
- [ ] Favorites/bookmarks
- [ ] Custom encryption algorithms
- [ ] Compression before encryption
- [ ] Archive format support

### Planned for v2.0

- [ ] Graphical improvements
- [ ] Plugin system
- [ ] Cloud sync (OneDrive, Google Drive)
- [ ] Team collaboration features
- [ ] Web interface
- [ ] Mobile app
- [ ] Decentralized sync

---

## How to Update

1. Download latest release from [GitHub Releases](https://github.com/yourusername/myVault/releases)
2. Replace old executable with new one
3. Configuration file (vault_config.json) is preserved
4. All encrypted files remain accessible

---

## Version History

| Version | Date | Status | Download |
|---------|------|--------|----------|
| 1.0.0 | 2025-11-02 | ✅ Released | [Download](https://github.com/yourusername/myVault/releases/tag/v1.0.0) |

---

## Reporting Issues

Found a bug? Please report it on [GitHub Issues](https://github.com/yourusername/myVault/issues)

Include:
- MyVault version
- Windows version
- File size and count
- Error message
- Steps to reproduce

---

## Contributing

Want to contribute? See [CONTRIBUTING.md](CONTRIBUTING.md)

---

## License

MyVault is licensed under the MIT License - see [LICENSE](LICENSE) file for details.

---

**Last Updated**: November 2, 2025
**Maintainer**: [Your Name]
**Repository**: https://github.com/yourusername/myVault
