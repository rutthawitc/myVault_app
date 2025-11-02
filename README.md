# MyVault 🔒

Secure file encryption made simple. Military-grade encryption for your sensitive files.

## Features

- ✅ **ChaCha20-Poly1305 AEAD encryption** - Military-grade authenticated encryption
- ✅ **Batch operations** - Encrypt/decrypt 99+ files (14GB+ tested)
- ✅ **Streaming mode** - Constant memory usage regardless of file size
- ✅ **Custom lock icon** - Professional UI with visual lock indicator
- ✅ **Multi-select** - Shift+click range selection for bulk operations
- ✅ **Real-time timing** - See execution time on each operation
- ✅ **Cross-platform ready** - Windows (v1.0), macOS/Linux planned (v1.1)

## Quick Start

1. Download latest release from [GitHub Releases](https://github.com/rutthawitc/myVault_app/releases)
2. Run `my_vault.exe`
3. Create your master password
4. Start encrypting your files!

See [QUICKSTART.md](docs/QUICKSTART.md) for detailed setup instructions.

## Download

**Latest Release**: [MyVault v1.0.0](https://github.com/rutthawitc/myVault_app/releases/tag/v1.0.0)

Available as:
- Portable executable (10 MB)
- Windows installer (.msi)
- Portable USB setup

## Documentation

- 📖 [Quick Start Guide](docs/QUICKSTART.md) - 60-second setup
- 📚 [Deployment Guide](docs/DEPLOYMENT_GUIDE.md) - Complete reference
- 💾 [Installation Methods](docs/INSTALLER_GUIDE.md) - 5 different ways
- 📋 [Changelog](CHANGELOG.md) - Version history and features
- 🤝 [Contributing](CONTRIBUTING.md) - How to contribute
- 📜 [License](LICENSE) - MIT License

## Security Features

- **Military-grade encryption**: ChaCha20-Poly1305 AEAD authenticated encryption
- **Brute-force resistant**: Argon2id password hashing
- **Memory-safe**: Written in Rust (prevents buffer overflows)
- **Proper cleanup**: Automatic secure resource deallocation
- **No temporary files**: All operations are in-memory encrypted

## Performance

### Tested Throughput
- 1 MB file: ~100ms
- 100 MB file: 1-2 seconds
- 1 GB file: 15-30 seconds
- 10 GB file: 2-5 minutes
- **99 files (14GB total): ~2 minutes** ✅

### Memory Usage
- Constant memory (streaming mode)
- Per-operation: 48MB max
- Scales to any file size

## Building

```bash
# Clone the repository
git clone https://github.com/rutthawitc/myVault_app.git
cd myVault_app

# Build release version
cargo build --release

# Output: target/release/my_vault.exe
```

## Testing

```bash
# Run all tests
cargo test --all

# Build and test together
cargo build --release && cargo test --release
```

## System Requirements

- **OS**: Windows 7 or later
- **RAM**: 512 MB minimum (2GB recommended)
- **Storage**: 10 MB free space for executable

## How It Works

1. **Create Master Password**: One password to encrypt/decrypt all files
2. **Select Files**: Add files or folders to encrypt
3. **Lock/Unlock**: One-click batch encryption/decryption
4. **Automatic Cleanup**: Files are securely replaced after operation

## Roadmap

### v1.0 ✅ (Current)
- Windows file encryption
- Batch operations (99+ files tested)
- Multi-select support
- Real-time execution timing

### v1.1 (Planned)
- macOS support
- Linux support
- Drag and drop
- Custom chunk size
- Pause/resume operations

### v1.2 (Planned)
- Directory encryption
- Password strength meter
- Recent files list
- Custom algorithms

### v2.0 (Planned)
- Cloud sync
- Team collaboration
- Web interface
- Mobile app

## Community

- 💬 [Discussions](https://github.com/rutthawitc/myVault_app/discussions) - Ask questions, share ideas
- 🐛 [Issues](https://github.com/rutthawitc/myVault_app/issues) - Report bugs or request features
- 🤝 [Contributing](CONTRIBUTING.md) - Help improve MyVault

## Code of Conduct

This project adheres to the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## License

MyVault is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

This means you can:
- ✅ Use commercially
- ✅ Modify the source
- ✅ Distribute copies
- ✅ Private use

Just include the original license with your distribution.

## Support

Found an issue? Have a suggestion?

- 🐛 [Report a bug](https://github.com/rutthawitc/myVault_app/issues/new?template=bug_report.md)
- 💡 [Request a feature](https://github.com/rutthawitc/myVault_app/issues/new?template=feature_request.md)
- 💬 [Start a discussion](https://github.com/rutthawitc/myVault_app/discussions/new)

## Security Vulnerability Disclosure

If you discover a security vulnerability, please email security@example.com instead of using the issue tracker. See [SECURITY.md](docs/SECURITY.md) for details.

---

**Status**: ✅ Production Ready (v1.0.0)

**Last Updated**: November 2, 2025

**Repository**: https://github.com/rutthawitc/myVault_app

Made with ❤️ for secure file encryption
