# Contributing to MyVault

Thank you for your interest in contributing to MyVault! We welcome contributions from the community.

## Code of Conduct

Please review our [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing.

## How to Contribute

### 1. Reporting Bugs 🐛

Found a bug? Please open an issue using our [Bug Report Template](https://github.com/yourusername/myVault/issues/new?template=bug_report.md)

Include:
- **Description**: Clear explanation of the bug
- **Steps to Reproduce**: Exact steps to reproduce the issue
- **Expected Behavior**: What should happen
- **Actual Behavior**: What actually happens
- **System Info**: Windows version, RAM, file sizes, error messages
- **Screenshots**: If applicable

### 2. Requesting Features 💡

Have a great idea? Open an issue using our [Feature Request Template](https://github.com/yourusername/myVault/issues/new?template=feature_request.md)

Include:
- **Description**: Clear description of the feature
- **Use Case**: Why this feature would be useful
- **Examples**: Similar features or mockups
- **Additional Context**: Any relevant information

### 3. Submitting Code ✅

Ready to code? Follow these steps:

#### Step 1: Fork the Repository
```bash
# Visit: https://github.com/yourusername/myVault
# Click "Fork" button
```

#### Step 2: Clone Your Fork
```bash
git clone https://github.com/YOUR_USERNAME/myVault.git
cd myVault
```

#### Step 3: Create Feature Branch
```bash
git checkout -b feature/my-feature-name
# Or for bug fixes:
git checkout -b fix/bug-description
```

#### Step 4: Make Changes
- Edit files
- Add tests for new features
- Update documentation if needed

#### Step 5: Test Your Changes
```bash
# Run all tests
cargo test --all

# Check code formatting
cargo fmt --all

# Check for issues with Clippy
cargo clippy -- -D warnings

# Build release
cargo build --release
```

#### Step 6: Commit Changes
```bash
# Make sure your commit message is clear
git commit -am "Add my feature

- What changed
- Why it changed
- Any related issues: Fixes #123"
```

#### Step 7: Push to Your Fork
```bash
git push origin feature/my-feature-name
```

#### Step 8: Create Pull Request
1. Visit your fork on GitHub
2. Click "Compare & pull request"
3. Fill in PR template
4. Submit!

## Pull Request Guidelines

### PR Template
Your PR should include:
- **Description**: What changes are being made?
- **Type**: Bug fix / New feature / Documentation / Performance
- **Testing**: How was this tested?
- **Checklist**: Code style, tests, documentation

### PR Requirements
- [ ] Code follows Rust style guidelines (run `cargo fmt`)
- [ ] Tests added/updated for changes
- [ ] Documentation updated if needed
- [ ] No breaking changes (or clearly documented)
- [ ] Passes all CI/CD checks

## Code Style Guidelines

### Rust Style
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Format with `cargo fmt`
- Lint with `cargo clippy`

### Comments
```rust
// Good comment - explains WHY, not WHAT
// This uses streaming mode to prevent memory exhaustion on large files
let mut buffer = vec![0u8; CHUNK_SIZE];

// Avoid obvious comments
// Increment counter
counter += 1;
```

### Error Handling
```rust
// Good: Clear error messages
Err("Invalid file header".to_string())

// Better: Context-specific errors
Err(format!("Failed to decrypt chunk {}: {}", chunk_index, e))
```

### Documentation
```rust
/// Encrypts a file using streaming mode
///
/// This function reads the input file in 16MB chunks and encrypts
/// each chunk independently, which prevents memory exhaustion on
/// large files.
///
/// # Arguments
/// * `key_bytes` - 32-byte encryption key
/// * `input_path` - Path to file to encrypt
/// * `output_path` - Path for encrypted file
///
/// # Errors
/// Returns error if file cannot be read/written or encryption fails
pub fn encrypt_file_streaming(
    key_bytes: &[u8; 32],
    input_path: &Path,
    output_path: &Path,
) -> Result<(), String>
```

## Testing

### Unit Tests
```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_encrypt_decrypt

# Run with output
cargo test -- --nocapture
```

### Adding Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32];
        let plaintext = b"Hello, World!";

        let encrypted = encrypt_blob(&key, plaintext).unwrap();
        let decrypted = decrypt_blob(&key, &encrypted).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }
}
```

## Documentation

### README.md
- Clear description
- Quick start guide
- Feature list
- Link to detailed docs

### Code Comments
- Explain WHY, not WHAT
- Document complex logic
- Add examples for public APIs

### Commit Messages
```
# Good format
Add feature X

- What was added
- Why it was needed
- Any related issues: Fixes #123

# Good example
Add shift+click range selection

- Users can now select ranges of files
- Improves batch operation usability
- Requested in #45
```

## Review Process

1. **Submission**: PR submitted with description
2. **Automated Tests**: GitHub Actions runs tests
3. **Code Review**: Maintainers review code
4. **Feedback**: Comments for improvements
5. **Revision**: Author makes requested changes
6. **Approval**: Approved and ready to merge
7. **Merge**: Code merged to main branch

## Development Setup

### Requirements
- Rust 1.70+ (install from https://rustup.rs/)
- Cargo (included with Rust)
- Git

### Build
```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Check code
cargo clippy
cargo fmt --check
```

## Release Process

Releases follow semantic versioning:
- `v1.0.0` - Initial release
- `v1.1.0` - New features
- `v1.1.1` - Bug fixes
- `v2.0.0` - Breaking changes

Release cycle:
1. Update version in Cargo.toml
2. Update CHANGELOG.md
3. Create git tag: `git tag v1.x.x`
4. Push tag: `git push origin v1.x.x`
5. GitHub Actions builds and uploads binary
6. Create Release on GitHub

## Questions?

- **General Questions**: Open a GitHub Discussion
- **Bug Reports**: Create an Issue
- **Feature Ideas**: Create an Issue with "enhancement" label
- **Security Issues**: Email (don't create public issue)

## Recognition

Contributors are recognized in:
- README.md (all contributors)
- CHANGELOG.md (release notes)
- GitHub Contributors page

## License

By contributing to MyVault, you agree that your contributions will be licensed under the MIT License.

---

**Thank you for contributing to MyVault!** 🙏

We appreciate your efforts to make MyVault better for everyone.
