# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Commands

| Task | Command |
|------|---------|
| **Build** | `cargo build --release` |
| **Test all** | `cargo test --all` |
| **Run tests + build** | `cargo build --release && cargo test --release` |
| **Test single module** | `cargo test --lib crypto::tests --release` |
| **Lint warnings** | `cargo clippy` |
| **Run application** | `cargo run --release` |
| **Check compilation** | `cargo check` |

## Project Overview

**MyVault** is a secure file encryption application written in Rust using military-grade cryptography (ChaCha20-Poly1305 AEAD). It provides a cross-platform GUI (egui) for encrypting/decrypting files with a master password.

**Key Characteristics**:
- Version: 1.0.0 (production-ready)
- Encryption: ChaCha20-Poly1305 AEAD with Argon2id key derivation
- Platform: Windows (v1.0), macOS/Linux planned
- Scale: Handles 99+ files (~14GB) in batch operations
- Memory usage: Constant (streaming mode, 16MB chunks)
- GUI Framework: egui (immediate-mode UI)

## Architecture Overview

### High-Level Design

The application follows a **layered, modular architecture**:

```
┌────────────────────────────────────────┐
│   egui GUI Layer (main.rs)             │  - UI state, batch operations
│   Application state & event handling   │  - Multi-file operations
└────────────────────────────────────────┘
              ↓
    ┌─────────┴──────────┬──────────┐
    │                    │          │
┌───▼────────┐  ┌────────▼───┐  ┌──▼──────────┐
│ Crypto     │  │ Config &   │  │ Performance │
│ (crypto.rs)│  │ Persistence│  │ & Platform  │
│            │  │ (config.rs)│  │ Modules     │
└────────────┘  └────────────┘  └─────────────┘
    ↓                                 ↓
┌────────────────────────────────────────┐
│ Streaming I/O                          │  - 16MB chunk processing
│ - one worker thread per file           │  - max 4 worker threads
│ - File I/O with 32MB buffers           │  - CPU core detection
└────────────────────────────────────────┘
```

### Key Modules

| Module | Purpose | Key Details |
|--------|---------|------------|
| **main.rs** | UI & Application State | egui event loop, batch operations, file list management |
| **crypto.rs** | Encryption/Decryption | ChaCha20-Poly1305 streaming (chunks), STREAM construction, roundtrip tests |
| **config.rs** | JSON Persistence | Vault items, UI preferences, security settings storage |
| **performance.rs** | Thread Sizing | CPU core detection, worker-thread count |
| **platform.rs** | Platform-Specific APIs | File attributes (Windows), config directory paths (cross-platform) |
| **model.rs** | Data Structures | VaultItem, ItemType enums, serialization |

### Architectural Patterns

1. **Chunked Streaming**: Files processed in 16MB chunks to ensure O(1) memory regardless of file size
2. **Bounded Parallelism**: Max 4 worker threads, one file per thread (prevents memory exhaustion)
3. **Channel-Based Threading**: Background threads communicate via `mpsc` channels; UI stays responsive
5. **Configuration Persistence**: `vault_config.json` stored in platform-specific app directories
6. **Secure Memory Handling**: All sensitive data (passwords, keys) explicitly zeroed using `zeroize` crate

## Critical Constraints & Requirements

### Encryption & Cryptography
- **Always use ChaCha20-Poly1305** for AEAD operations (no exceptions)
- **Argon2id parameters**: Must match existing config in code (memory, time, parallelism)
- **Nonce generation**: Must use `OsRng` for cryptographically secure randomness
- **Memory cleanup**: All password buffers and key material must be `zeroize`d after use
- **Testing**: All crypto changes require roundtrip tests (encrypt → decrypt → verify integrity)

### Performance & Parallelism
- **Max worker threads**: Hard limit of 4 threads (prevents memory exhaustion during batch operations)
- **Chunk size**: 16MB is the standard (don't change without benchmarking all scenarios)
- **Buffer sizes**: 32MB I/O buffers for streaming operations
- **CPU detection**: Use `num_cpus` crate to determine thread pool size

### Configuration & Persistence
- **Config path**: Platform-aware (use `dirs` crate):
  - Windows: `%APPDATA%\MyVault\vault_config.json`
  - macOS: `~/Library/Application Support/MyVault/vault_config.json`
  - Linux: `~/.local/share/myvault/vault_config.json`
- **Format**: JSON serializable structs with `serde`
- **Contents**: Argon2 hash of master password, vault items list, UI preferences, security settings

### UI & Event Loop
- **Framework**: egui (immediate-mode, cross-platform)
- **Responsiveness**: All blocking operations must run in background threads
- **Progress feedback**: Batch progress is tracked on `BatchOp` (processed / failures / start_time)
- **File dialogs**: Use `rfd` crate for native file/folder selection
- **No blocking**: Main UI thread must never block on I/O or crypto operations

## Development Workflow

### Before Making Changes
1. **Understand the context**: Read the relevant module(s) before editing
2. **Check constraints**: Review the critical constraints section above
3. **Plan for tests**: If modifying crypto or performance code, plan test cases

### During Development
- **Crypto changes**: Write roundtrip tests for various file sizes (1MB, 100MB, 1GB+)
- **Performance changes**: Benchmark with 50+ files
- **UI changes**: Test responsiveness with large batch operations
- **Memory safety**: Use `cargo check` and `cargo clippy` frequently

### Testing Strategy
- **Crypto**: `cargo test --lib crypto::tests --release` (includes 1MB, 128MB, large file tests)
- **Performance**: `cargo test --lib performance::tests --release` (CPU detection, thread count)
- **Integration**: `cargo test --all --release` (full system testing)
- **Manual**: `cargo run --release` and test with actual files (UI responsiveness, correctness)

## Important Code Locations

> Referenced by symbol rather than line number, so these stay correct as the code moves.

| Task | Location |
|------|----------|
| Batch operation thread spawning | `MyVaultApp::update` in `src/main.rs` (the `current_op` block) |
| Streaming encryption / decryption | `crypto::encrypt_file_streaming` / `crypto::decrypt_file_streaming` |
| Legacy format reading (V1) | `crypto::decrypt_file_streaming_v1` |
| Worker-thread count | `src/performance.rs` |
| Configuration saving / loading | `config::save_config` / `config::load_config` |
| Encrypted filename mapping | `MyVaultApp::encrypted_path_for` / `original_path_for` |
| Crypto test cases | `mod tests` at the end of `src/crypto.rs` |

## Common Development Tasks

### Adding a New Encryption Algorithm
1. Update `crypto.rs` with new algorithm implementation
2. Add test cases covering multiple file sizes
3. Update performance module if algorithm has different memory profile
4. Ensure all sensitive data is `zeroize`d

### Fixing Performance Issues
1. Check `performance.rs` for current CPU/memory detection strategy
2. Review thread pool size and chunk size in batch operations
3. Test with 50+ files and measure throughput
4. Verify memory stays under 48MB per operation
5. Consider storage type (SSD prefetch may not help HDD)

### Modifying the UI
1. Changes go in `src/main.rs` (UI event loop starts around line 633)
2. Keep all blocking operations in background threads
3. Use `rfd` for file dialogs, never hardcode paths
4. Test responsiveness with large batch operations

### Supporting a New Platform
1. Add platform detection in `Cargo.toml` (see `[target.'cfg(windows)'.dependencies]`)
2. Implement platform-specific code in `src/platform.rs`
3. Update config paths in `platform.rs` using `dirs` crate
4. Update build scripts (`build-*.sh`)
5. Test on target platform

## Dependencies Overview

### Core Cryptography
- **chacha20poly1305** (0.10): AEAD encryption primitive
- **argon2** (0.5) + **password-hash** (0.5): Key derivation
- **rand** (0.8): Secure random number generation
- **zeroize** (1.7): Secure memory cleanup

### Concurrency & Performance
- **num_cpus** (1.16): CPU core detection

### UI & Files
- **eframe** (0.32) + **egui**: Cross-platform immediate-mode GUI
- **rfd** (0.14): Native file/folder dialogs
- **walkdir** (2.5): Recursive directory traversal

### Persistence & Platform
- **serde** + **serde_json** (1.0): JSON serialization
- **dirs** (5.0): Platform-aware config paths
- **winapi** (0.3): Windows-specific APIs (file attributes, clipboard)

## Git & Contribution Workflow

- **Main branch**: `main` (always deployable)
- **Commit messages**: Reference issue numbers, use conventional commits when possible
- **Testing**: All changes must pass `cargo test --all --release`
- **Linting**: Address `cargo clippy` warnings before committing

## Phase Development Status

- **Phase 1** ✅: Quick wins (password strength, dark mode, clipboard, throughput)
- **Phase 2** ✅: UX improvements (drag-drop, search, file sizes, sorting)
- **Phase 3** ✅: Security & persistence (session timeout, password reminders, recent files, password generator)
- **Phase 4** 🔄: Advanced features (pause/resume, custom chunk sizes, directory encryption)

Refer to `PHASE*.md` files for detailed implementation notes on completed phases.
