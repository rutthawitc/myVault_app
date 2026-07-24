use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use password_hash::{rand_core::OsRng, SaltString};
use rand::RngCore;
use zeroize::Zeroize;

pub fn hash_password(password: &str) -> Result<(String, String), String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    Ok((hash, salt.as_str().to_string()))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| e.to_string())?;
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(true),
        Err(password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

// Derive a 32-byte key from password + salt using Argon2id.
pub fn derive_key(password: &str, salt_str: &str) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();
    // For simplicity and determinism, we feed the ASCII salt string bytes.
    // This matches how we store the salt and is sufficient for MVP.
    argon2
        .hash_password_into(password.as_bytes(), salt_str.as_bytes(), &mut key)
        .map_err(|e| e.to_string())?;
    Ok(key)
}

// Envelope encryption (DEK/KEK)
//
// The Data Encryption Key (DEK) is a random 32-byte key that encrypts every file
// and NEVER changes for the lifetime of the vault. The Key Encryption Key (KEK) is
// derived from the master password via Argon2id and is only used to wrap the DEK.
//
// Changing the master password re-wraps the same DEK with a new KEK, so previously
// encrypted files remain decryptable. Without this indirection, a password change
// would derive a brand-new file key and permanently orphan every existing file.

const WRAPPED_DEK_LEN: usize = NONCE_LEN + 32 + 16; // nonce + key + Poly1305 tag

/// Generate a fresh random Data Encryption Key.
pub fn generate_dek() -> [u8; 32] {
    let mut dek = [0u8; 32];
    OsRng.fill_bytes(&mut dek);
    dek
}

/// Wrap (encrypt) the DEK with the KEK derived from the master password.
///
/// Returns a hex string safe to store in `vault_config.json`.
pub fn wrap_dek(kek: &[u8; 32], dek: &[u8; 32]) -> Result<String, String> {
    let key = Key::from(*kek);
    let cipher = ChaCha20Poly1305::new(&key);

    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let nonce_ref = Nonce::from(nonce);

    let mut ciphertext = cipher
        .encrypt(&nonce_ref, dek.as_ref())
        .map_err(|_| "failed to wrap data encryption key".to_string())?;

    let mut blob = Vec::with_capacity(WRAPPED_DEK_LEN);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    ciphertext.zeroize();

    Ok(to_hex(&blob))
}

/// Unwrap (decrypt) the DEK using the KEK derived from the master password.
///
/// Fails if the password is wrong or the stored blob was tampered with.
pub fn unwrap_dek(kek: &[u8; 32], wrapped_hex: &str) -> Result<[u8; 32], String> {
    let blob = from_hex(wrapped_hex)?;
    if blob.len() != WRAPPED_DEK_LEN {
        return Err("stored key blob has invalid length".to_string());
    }

    let key = Key::from(*kek);
    let cipher = ChaCha20Poly1305::new(&key);

    let nonce_array = <[u8; NONCE_LEN]>::try_from(&blob[..NONCE_LEN])
        .map_err(|_| "invalid nonce".to_string())?;
    let nonce_ref = Nonce::from(nonce_array);

    let mut plaintext = cipher
        .decrypt(&nonce_ref, &blob[NONCE_LEN..])
        .map_err(|_| "failed to unwrap data encryption key".to_string())?;

    let dek = <[u8; 32]>::try_from(plaintext.as_slice())
        .map_err(|_| "unwrapped key has invalid length".to_string())?;
    plaintext.zeroize();

    Ok(dek)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("stored key blob is malformed".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "stored key blob is malformed".to_string()))
        .collect()
}

// File format headers
const HEADER_V1: &[u8] = b"MYVAULTv1\n";  // Legacy: single-pass encryption (read-only)
const HEADER_V2: &[u8] = b"MYVAULTv2\n";  // Legacy: chunked AEAD, no AAD binding (read-only)
const HEADER_V3: &[u8] = b"MYVAULTv3\n";  // Current: chunked AEAD with authenticated framing
const NONCE_LEN: usize = 12;               // Per-chunk nonce size (ChaCha20)
const MASTER_NONCE_LEN: usize = 24;        // Master nonce for chunk derivation
const CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16MB chunks (balance between performance and memory with parallel ops)
const TAG_LEN: usize = 16;                 // Poly1305 authentication tag
/// Largest ciphertext a single chunk can legitimately produce. Chunk lengths are
/// read from the file itself, so they must be bounded before allocating.
const MAX_CHUNK_CIPHERTEXT: usize = CHUNK_SIZE + TAG_LEN;

/// Chunk framing flags (V3). Stored in the clear but authenticated via AAD.
const FLAG_FINAL: u8 = 0b0000_0001;

/// A 32-byte key that is wiped from memory when dropped.
///
/// Deliberately not `Copy`: every copy of the key must be tracked so it can be
/// zeroized. Clone when a background thread needs its own handle.
pub struct SecretKey([u8; 32]);

impl SecretKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Clone for SecretKey {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Build the additional authenticated data that binds a chunk to its position.
///
/// Including the format header, the chunk index and the framing flags means an
/// attacker cannot reorder chunks, splice chunks between files, or flip the
/// "final chunk" marker without the authentication tag failing.
fn chunk_aad(index: u64, flags: u8) -> [u8; HEADER_AAD_LEN] {
    let mut aad = [0u8; HEADER_AAD_LEN];
    aad[..HEADER_V3.len()].copy_from_slice(HEADER_V3);
    aad[HEADER_V3.len()..HEADER_V3.len() + 8].copy_from_slice(&index.to_le_bytes());
    aad[HEADER_V3.len() + 8] = flags;
    aad
}

const HEADER_AAD_LEN: usize = 10 + 8 + 1; // header + chunk index + flags

/// Check if a file is encrypted by MyVault (has the magic header)
pub fn is_encrypted_file(path: &std::path::Path) -> bool {
    use std::fs::File;
    use std::io::Read;

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut header = vec![0u8; HEADER_V3.len()];
    match file.read_exact(&mut header) {
        Ok(_) => header == HEADER_V1 || header == HEADER_V2 || header == HEADER_V3,
        Err(_) => false,
    }
}

/// Read until the buffer is full or the input is exhausted.
///
/// `Read::read` is allowed to return fewer bytes than requested even mid-file, so
/// a single call cannot be used to detect end-of-file.
fn fill_buffer(input: &mut impl std::io::Read, buf: &mut [u8]) -> Result<usize, String> {
    let mut filled = 0;
    while filled < buf.len() {
        match input.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(filled)
}

/// Derive a unique nonce for each chunk using XOR with chunk index
///
/// This implements the STREAM construction pattern where each chunk gets
/// a deterministic but unique nonce derived from a master nonce.
fn derive_chunk_nonce(master: &[u8; MASTER_NONCE_LEN], index: u64) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    let index_bytes = index.to_le_bytes();

    // XOR first 8 bytes of nonce with chunk index
    for i in 0..8 {
        nonce[i] = master[i] ^ index_bytes[i];
    }

    // Keep remaining nonce bytes from master
    nonce[8..NONCE_LEN].copy_from_slice(&master[8..NONCE_LEN]);

    nonce
}

/// Encrypt a file using true chunked AEAD (STREAM construction)
///
/// This processes files in 64MB chunks, each encrypted independently with
/// a unique derived nonce. This allows:
/// - Memory efficient: Only 64MB in memory at a time
/// - Streaming: Encrypts as data flows
/// - Parallel-ready: Chunks can be encrypted in parallel (Phase 2)
///
/// Format (V3):
/// [HEADER_V3(10)][MASTER_NONCE(24)][CHUNK_1][CHUNK_2]...[CHUNK_N]
/// Where each chunk is: [FLAGS(1)][LENGTH(8)][CIPHERTEXT+TAG(length bytes)]
///
/// The flags byte marks the last chunk and is authenticated as AAD together with
/// the chunk index, so truncating, reordering or splicing chunks is detected.
pub fn encrypt_file_streaming(
    key_bytes: &[u8; 32],
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<(), String> {
    use chacha20poly1305::aead::Payload;
    use std::fs::File;
    use std::io::{Write, BufWriter};

    let mut input = File::open(input_path).map_err(|e| e.to_string())?;
    let output_file = File::create(output_path).map_err(|e| e.to_string())?;
    let mut output = BufWriter::with_capacity(32 * 1024 * 1024, output_file);  // 32MB buffer (reduced for parallel ops safety)

    // Write file header (identifies this as MyVault V3 format)
    output.write_all(HEADER_V3).map_err(|e| e.to_string())?;

    // Generate and write master nonce (24 bytes for future XChaCha20 compatibility)
    let mut master_nonce = [0u8; MASTER_NONCE_LEN];
    OsRng.fill_bytes(&mut master_nonce);
    output.write_all(&master_nonce).map_err(|e| e.to_string())?;

    let key = Key::from(*key_bytes);
    let mut chunk_index = 0u64;

    // Writes one framed, authenticated chunk
    let write_chunk = |output: &mut BufWriter<File>, plaintext: &[u8], index: u64, is_final: bool| -> Result<(), String> {
        let flags = if is_final { FLAG_FINAL } else { 0 };
        let chunk_nonce = derive_chunk_nonce(&master_nonce, index);
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce_ref = Nonce::from(chunk_nonce);
        let aad = chunk_aad(index, flags);

        let ciphertext = cipher
            .encrypt(&nonce_ref, Payload { msg: plaintext, aad: &aad })
            .map_err(|e| e.to_string())?;

        output.write_all(&[flags]).map_err(|e| e.to_string())?;
        output
            .write_all(&(ciphertext.len() as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        output.write_all(&ciphertext).map_err(|e| e.to_string())?;
        Ok(())
    };

    // One chunk of lookahead so the last chunk can be marked as final
    let mut pending: Option<Vec<u8>> = None;
    loop {
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let n = fill_buffer(&mut input, &mut buffer)?;
        if n == 0 {
            break;
        }
        buffer.truncate(n);

        if let Some(prev) = pending.take() {
            write_chunk(&mut output, &prev, chunk_index, false)?;
            chunk_index += 1;
        }
        pending = Some(buffer);
    }

    // Always emit a final chunk, even for an empty file, so decryption can prove
    // it reached the real end of the data.
    let last = pending.take().unwrap_or_default();
    write_chunk(&mut output, &last, chunk_index, true)?;

    // Flush the buffer, then fsync so the ciphertext is durable on disk.
    // The caller deletes the plaintext source right after this returns, so a
    // flush alone (which only reaches the OS page cache) is not enough: a crash
    // or power loss in between would destroy the original AND leave a truncated
    // ciphertext behind.
    output.flush().map_err(|e| e.to_string())?;
    let file = output.into_inner().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;

    // Explicitly drop files to release OS resources immediately
    drop(input);
    drop(file);

    Ok(())
}

/// Decrypt a file encrypted with chunked AEAD (STREAM construction)
///
/// Reads the master nonce and decrypts each chunk using derived nonces.
/// Supports V1 and V2 (legacy) formats for backward compatibility.
pub fn decrypt_file_streaming(
    key_bytes: &[u8; 32],
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<(), String> {
    use chacha20poly1305::aead::Payload;
    use std::fs::File;
    use std::io::{Read, Write, BufWriter};

    let mut input = File::open(input_path).map_err(|e| e.to_string())?;
    let output_file = File::create(output_path).map_err(|e| e.to_string())?;
    let mut output = BufWriter::with_capacity(32 * 1024 * 1024, output_file);  // 32MB buffer (reduced for parallel ops safety)

    // Read header to detect format
    let mut header_buf = vec![0u8; HEADER_V3.len()];
    input.read_exact(&mut header_buf).map_err(|e| e.to_string())?;

    let is_v3 = header_buf == HEADER_V3;
    if header_buf == HEADER_V1 {
        // Old V1 format: single-pass encryption
        // Re-open and use V1 decryption
        drop(input);
        return decrypt_file_streaming_v1(key_bytes, input_path, output_path);
    } else if !is_v3 && header_buf != HEADER_V2 {
        return Err("invalid file header".to_string());
    }

    // Read master nonce
    let mut master_nonce = [0u8; MASTER_NONCE_LEN];
    input.read_exact(&mut master_nonce).map_err(|e| e.to_string())?;

    let key = Key::from(*key_bytes);
    let mut chunk_index = 0u64;
    let mut saw_final_chunk = false;

    // Read and decrypt chunks
    loop {
        // V3 chunks start with an authenticated flags byte
        let mut flags = 0u8;
        if is_v3 {
            let mut flag_buf = [0u8; 1];
            match input.read_exact(&mut flag_buf) {
                Ok(_) => flags = flag_buf[0],
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.to_string()),
            }
            if saw_final_chunk {
                return Err("extra data after the end of the file".to_string());
            }
        }

        // Read chunk length
        let mut len_bytes = [0u8; 8];
        match input.read_exact(&mut len_bytes) {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                if is_v3 {
                    // A flags byte was read, so the chunk header is cut off
                    return Err("file is truncated or corrupted".to_string());
                }
                // Legacy V2: end of file is the only end marker available
                break;
            }
            Err(e) => return Err(e.to_string()),
        }

        let chunk_len = u64::from_le_bytes(len_bytes) as usize;
        // Bound the length before allocating: it comes straight from the file and
        // an absurd value would otherwise blow up memory.
        if !(TAG_LEN..=MAX_CHUNK_CIPHERTEXT).contains(&chunk_len) {
            return Err("file is truncated or corrupted".to_string());
        }

        // Read ciphertext for this chunk
        let mut ciphertext = vec![0u8; chunk_len];
        input
            .read_exact(&mut ciphertext)
            .map_err(|_| "file is truncated or corrupted".to_string())?;

        // Derive nonce for this chunk and decrypt
        let chunk_nonce = derive_chunk_nonce(&master_nonce, chunk_index);
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce_ref = Nonce::from(chunk_nonce);

        let plaintext = if is_v3 {
            let aad = chunk_aad(chunk_index, flags);
            cipher.decrypt(&nonce_ref, Payload { msg: ciphertext.as_ref(), aad: &aad })
        } else {
            cipher.decrypt(&nonce_ref, ciphertext.as_ref())
        }
        .map_err(|_| format!("Failed to decrypt chunk {} (wrong password or corrupted file)", chunk_index))?;

        output.write_all(&plaintext).map_err(|e| e.to_string())?;

        if is_v3 && flags & FLAG_FINAL != 0 {
            saw_final_chunk = true;
        }
        chunk_index += 1;
    }

    // V3 files always carry an authenticated end marker. If it is missing the
    // file was cut short - refuse rather than silently returning partial data.
    if is_v3 && !saw_final_chunk {
        return Err("file is truncated: the end-of-file marker is missing".to_string());
    }

    // Flush + fsync before the caller removes the encrypted source file
    output.flush().map_err(|e| e.to_string())?;
    let file = output.into_inner().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;

    // Explicitly drop files to release OS resources immediately
    drop(input);
    drop(file);

    Ok(())
}

/// Decrypt V1 format files (single-pass encryption)
///
/// Backward compatibility: supports old file format for migration period
fn decrypt_file_streaming_v1(
    key_bytes: &[u8; 32],
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::{Read, Write, Seek, SeekFrom};

    let mut input = File::open(input_path).map_err(|e| e.to_string())?;
    let mut output = File::create(output_path).map_err(|e| e.to_string())?;

    // Skip header (already read)
    let _ = input.seek(SeekFrom::Start(HEADER_V1.len() as u64));

    // Read nonce
    let mut nonce = [0u8; NONCE_LEN];
    input.read_exact(&mut nonce).map_err(|e| e.to_string())?;

    // Read all ciphertext (this is the old way - accumulates in memory)
    let mut ciphertext = Vec::new();
    input.read_to_end(&mut ciphertext).map_err(|e| e.to_string())?;

    // Decrypt
    let key = Key::from(*key_bytes);
    let cipher = ChaCha20Poly1305::new(&key);
    let nonce_ref = Nonce::from(nonce);

    let plaintext = cipher
        .decrypt(&nonce_ref, ciphertext.as_ref())
        .map_err(|e| e.to_string())?;

    output.write_all(&plaintext).map_err(|e| e.to_string())?;

    // fsync before the caller removes the encrypted source file
    output.sync_all().map_err(|e| e.to_string())?;

    // Explicitly drop files to release OS resources immediately
    drop(input);
    drop(output);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    /// Helper function to create a test file with specific size
    fn create_test_file(path: &Path, size: usize) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = fs::File::create(path)?;
        let chunk = vec![0xAB; 1024 * 1024];  // 1MB chunks of test data
        let mut remaining = size;

        while remaining > 0 {
            let to_write = remaining.min(chunk.len());
            file.write_all(&chunk[..to_write])?;
            remaining -= to_write;
        }

        Ok(())
    }

    #[test]
    fn test_small_file_encryption() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x42u8; 32];
        let input = Path::new("test_small_input.bin");
        let encrypted = Path::new("test_small_encrypted.vault");
        let decrypted = Path::new("test_small_decrypted.bin");

        // Create a small test file (1MB)
        create_test_file(input, 1024 * 1024)?;
        let original_data = fs::read(input)?;

        // Encrypt
        encrypt_file_streaming(&key, input, encrypted)?;

        // Verify encrypted file is larger (due to length headers and auth tags)
        let encrypted_size = fs::metadata(encrypted)?.len();
        assert!(encrypted_size > original_data.len() as u64);

        // Decrypt
        decrypt_file_streaming(&key, encrypted, decrypted)?;
        let decrypted_data = fs::read(decrypted)?;

        // Verify decryption matches original
        assert_eq!(original_data, decrypted_data, "Decrypted data should match original");

        // Cleanup
        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;

        Ok(())
    }

    #[test]
    fn test_multiple_chunks() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0xCDu8; 32];
        let input = Path::new("test_chunks_input.bin");
        let encrypted = Path::new("test_chunks_encrypted.vault");
        let decrypted = Path::new("test_chunks_decrypted.bin");

        // Create file that spans exactly 2 chunks
        // Each chunk is 64MB, so create 128MB + 1MB
        let file_size = 129 * 1024 * 1024;
        create_test_file(input, file_size)?;
        let original_data = fs::read(input)?;

        // Encrypt
        encrypt_file_streaming(&key, input, encrypted)?;

        // Decrypt
        decrypt_file_streaming(&key, encrypted, decrypted)?;
        let decrypted_data = fs::read(decrypted)?;

        // Verify
        assert_eq!(original_data.len(), decrypted_data.len());
        assert_eq!(original_data, decrypted_data, "Multi-chunk roundtrip failed");

        // Cleanup
        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;

        Ok(())
    }

    /// Write a file in the legacy V2 format (no authenticated framing).
    /// Test-only: production code always writes V3.
    fn encrypt_file_v2_legacy(
        key_bytes: &[u8; 32],
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), String> {
        use std::io::{Read, Write};

        let mut input = fs::File::open(input_path).map_err(|e| e.to_string())?;
        let mut output = fs::File::create(output_path).map_err(|e| e.to_string())?;

        output.write_all(HEADER_V2).map_err(|e| e.to_string())?;
        let mut master_nonce = [0u8; MASTER_NONCE_LEN];
        OsRng.fill_bytes(&mut master_nonce);
        output.write_all(&master_nonce).map_err(|e| e.to_string())?;

        let key = Key::from(*key_bytes);
        let mut chunk_index = 0u64;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        loop {
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            let cipher = ChaCha20Poly1305::new(&key);
            let nonce_ref = Nonce::from(derive_chunk_nonce(&master_nonce, chunk_index));
            let ct = cipher
                .encrypt(&nonce_ref, &buffer[..n])
                .map_err(|e| e.to_string())?;
            output
                .write_all(&(ct.len() as u64).to_le_bytes())
                .map_err(|e| e.to_string())?;
            output.write_all(&ct).map_err(|e| e.to_string())?;
            chunk_index += 1;
        }
        Ok(())
    }

    #[test]
    fn test_file_format_v3() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0xDEu8; 32];
        let input = Path::new("test_format_input.bin");
        let encrypted = Path::new("test_format_encrypted.vault");

        // Create test file
        create_test_file(input, 10 * 1024 * 1024)?;

        // Encrypt
        encrypt_file_streaming(&key, input, encrypted)?;

        // Verify file starts with correct header
        let mut file = fs::File::open(encrypted)?;
        let mut header = vec![0u8; HEADER_V3.len()];
        std::io::Read::read_exact(&mut file, &mut header)?;
        assert_eq!(&header, HEADER_V3, "File should have V3 header");

        // Verify master nonce is present
        let mut master_nonce = vec![0u8; MASTER_NONCE_LEN];
        std::io::Read::read_exact(&mut file, &mut master_nonce)?;
        // Master nonce should not be all zeros
        assert!(!master_nonce.iter().all(|&b| b == 0), "Master nonce should be random");

        // Cleanup
        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;

        Ok(())
    }

    #[test]
    fn test_wrong_password() -> Result<(), Box<dyn std::error::Error>> {
        let key1 = [0x11u8; 32];
        let key2 = [0x22u8; 32];
        let input = Path::new("test_wrong_pass_input.bin");
        let encrypted = Path::new("test_wrong_pass_encrypted.vault");
        let decrypted = Path::new("test_wrong_pass_decrypted.bin");

        // Create test file
        create_test_file(input, 10 * 1024 * 1024)?;

        // Encrypt with key1
        encrypt_file_streaming(&key1, input, encrypted)?;

        // Try to decrypt with key2 (should fail)
        let result = decrypt_file_streaming(&key2, encrypted, decrypted);
        assert!(result.is_err(), "Decryption with wrong key should fail");

        // Cleanup
        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        if decrypted.exists() {
            fs::remove_file(decrypted)?;
        }

        Ok(())
    }

    /// Regression test: cutting chunks off the end of a file used to decrypt
    /// "successfully" and silently hand back truncated data.
    #[test]
    fn test_truncated_file_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x77u8; 32];
        let input = Path::new("test_trunc_input.bin");
        let encrypted = Path::new("test_trunc_encrypted.vault");
        let truncated = Path::new("test_trunc_cut.vault");
        let decrypted = Path::new("test_trunc_decrypted.bin");

        // Two chunks so the file can lose its tail and still look plausible
        create_test_file(input, 20 * 1024 * 1024)?;
        encrypt_file_streaming(&key, input, encrypted)?;

        // Drop the last 1MB (removes the authenticated final-chunk marker)
        let full = fs::read(encrypted)?;
        fs::write(truncated, &full[..full.len() - 1024 * 1024])?;

        let result = decrypt_file_streaming(&key, truncated, decrypted);
        assert!(result.is_err(), "Truncated file must not decrypt silently");
        assert!(
            result.unwrap_err().to_lowercase().contains("truncat"),
            "Error should tell the user the file was truncated"
        );

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(truncated)?;
        if decrypted.exists() {
            fs::remove_file(decrypted)?;
        }
        Ok(())
    }

    /// The "final chunk" flag is stored in the clear, so it must be authenticated:
    /// clearing it on the last chunk would otherwise hide a truncation.
    #[test]
    fn test_tampered_final_flag_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x88u8; 32];
        let input = Path::new("test_flag_input.bin");
        let encrypted = Path::new("test_flag_encrypted.vault");
        let tampered = Path::new("test_flag_tampered.vault");
        let decrypted = Path::new("test_flag_decrypted.bin");

        create_test_file(input, 1024)?;
        encrypt_file_streaming(&key, input, encrypted)?;

        // The single chunk's flags byte sits right after header + master nonce
        let mut bytes = fs::read(encrypted)?;
        let flags_offset = HEADER_V3.len() + MASTER_NONCE_LEN;
        assert_eq!(bytes[flags_offset], FLAG_FINAL, "Only chunk should be final");
        bytes[flags_offset] = 0; // pretend more chunks follow
        fs::write(tampered, &bytes)?;

        assert!(
            decrypt_file_streaming(&key, tampered, decrypted).is_err(),
            "Flipping the final-chunk flag must fail authentication"
        );

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(tampered)?;
        if decrypted.exists() {
            fs::remove_file(decrypted)?;
        }
        Ok(())
    }

    /// A corrupted length field must not trigger a huge allocation.
    #[test]
    fn test_absurd_chunk_length_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x99u8; 32];
        let input = Path::new("test_len_input.bin");
        let encrypted = Path::new("test_len_encrypted.vault");
        let corrupted = Path::new("test_len_corrupted.vault");
        let decrypted = Path::new("test_len_decrypted.bin");

        create_test_file(input, 1024)?;
        encrypt_file_streaming(&key, input, encrypted)?;

        let mut bytes = fs::read(encrypted)?;
        let len_offset = HEADER_V3.len() + MASTER_NONCE_LEN + 1;
        // Claim a 1 exabyte chunk
        bytes[len_offset..len_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        fs::write(corrupted, &bytes)?;

        assert!(
            decrypt_file_streaming(&key, corrupted, decrypted).is_err(),
            "An impossible chunk length must be rejected before allocating"
        );

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(corrupted)?;
        if decrypted.exists() {
            fs::remove_file(decrypted)?;
        }
        Ok(())
    }

    #[test]
    fn test_empty_file_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x5Au8; 32];
        let input = Path::new("test_empty_input.bin");
        let encrypted = Path::new("test_empty_encrypted.vault");
        let decrypted = Path::new("test_empty_decrypted.bin");

        fs::write(input, b"")?;
        encrypt_file_streaming(&key, input, encrypted)?;
        decrypt_file_streaming(&key, encrypted, decrypted)?;

        assert_eq!(fs::read(decrypted)?.len(), 0, "Empty file should round-trip");

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;
        Ok(())
    }

    /// Files encrypted by earlier versions must keep opening.
    /// Write a file in the original V1 format (single-pass, whole file in memory).
    /// Test-only: production code can no longer produce this format, but must
    /// still be able to read files locked by early versions of the app.
    fn write_v1_file(key_bytes: &[u8; 32], plaintext: &[u8], output_path: &Path) -> Result<(), String> {
        let key = Key::from(*key_bytes);
        let cipher = ChaCha20Poly1305::new(&key);
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let nonce_ref = Nonce::from(nonce);

        let mut out = Vec::new();
        out.extend_from_slice(HEADER_V1);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&cipher.encrypt(&nonce_ref, plaintext).map_err(|e| e.to_string())?);
        fs::write(output_path, out).map_err(|e| e.to_string())
    }

    #[test]
    fn test_legacy_v1_file_still_decrypts() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x4Du8; 32];
        let encrypted = Path::new("test_v1compat_encrypted.vault");
        let decrypted = Path::new("test_v1compat_decrypted.bin");
        let payload = b"secrets locked by an early version of MyVault";

        write_v1_file(&key, payload, encrypted)?;
        assert!(is_encrypted_file(encrypted), "V1 file should be recognised");

        decrypt_file_streaming(&key, encrypted, decrypted)?;
        assert_eq!(payload.to_vec(), fs::read(decrypted)?, "V1 file must still decrypt");

        // A wrong key must still be rejected on the legacy path
        assert!(decrypt_file_streaming(&[0x00u8; 32], encrypted, decrypted).is_err());

        fs::remove_file(encrypted)?;
        if decrypted.exists() {
            fs::remove_file(decrypted)?;
        }
        Ok(())
    }

    #[test]
    fn test_is_encrypted_file_rejects_other_files() -> Result<(), Box<dyn std::error::Error>> {
        let plain = Path::new("test_detect_plain.txt");
        let tiny = Path::new("test_detect_tiny.bin");

        fs::write(plain, b"just an ordinary text file, definitely not a vault")?;
        assert!(!is_encrypted_file(plain), "Plain file must not be detected as encrypted");

        // Shorter than the header: must not panic or misreport
        fs::write(tiny, b"MY")?;
        assert!(!is_encrypted_file(tiny), "Truncated file must not be detected as encrypted");

        assert!(
            !is_encrypted_file(Path::new("test_detect_does_not_exist.bin")),
            "Missing file must not be detected as encrypted"
        );

        fs::remove_file(plain)?;
        fs::remove_file(tiny)?;
        Ok(())
    }

    #[test]
    fn test_legacy_v2_file_still_decrypts() -> Result<(), Box<dyn std::error::Error>> {
        let key = [0x3Cu8; 32];
        let input = Path::new("test_v2compat_input.bin");
        let encrypted = Path::new("test_v2compat_encrypted.vault");
        let decrypted = Path::new("test_v2compat_decrypted.bin");

        create_test_file(input, 3 * 1024 * 1024)?;
        let original_data = fs::read(input)?;

        encrypt_file_v2_legacy(&key, input, encrypted)?;
        assert!(is_encrypted_file(encrypted), "V2 file should be recognised");

        decrypt_file_streaming(&key, encrypted, decrypted)?;
        assert_eq!(original_data, fs::read(decrypted)?, "V2 file must still decrypt");

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;
        Ok(())
    }

    #[test]
    fn test_secret_key_exposes_bytes_and_clones() {
        let key = SecretKey::new([0x2Fu8; 32]);
        assert_eq!(key.as_bytes(), &[0x2Fu8; 32]);
        let cloned = key.clone();
        assert_eq!(cloned.as_bytes(), key.as_bytes());
    }

    #[test]
    fn test_dek_wrap_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let kek = derive_key("correct horse battery staple", "somesaltvalue")?;
        let dek = generate_dek();

        let wrapped = wrap_dek(&kek, &dek)?;
        let unwrapped = unwrap_dek(&kek, &wrapped)?;

        assert_eq!(dek, unwrapped, "Unwrapped DEK should match the original");
        Ok(())
    }

    #[test]
    fn test_dek_unwrap_rejects_wrong_password() -> Result<(), Box<dyn std::error::Error>> {
        let kek = derive_key("right-password", "somesaltvalue")?;
        let wrong_kek = derive_key("wrong-password", "somesaltvalue")?;
        let dek = generate_dek();

        let wrapped = wrap_dek(&kek, &dek)?;
        assert!(
            unwrap_dek(&wrong_kek, &wrapped).is_err(),
            "Unwrapping with the wrong password must fail"
        );
        Ok(())
    }

    #[test]
    fn test_dek_unwrap_rejects_tampered_blob() -> Result<(), Box<dyn std::error::Error>> {
        let kek = derive_key("a-password", "somesaltvalue")?;
        let dek = generate_dek();
        let wrapped = wrap_dek(&kek, &dek)?;

        // Flip one hex character in the ciphertext
        let mut chars: Vec<char> = wrapped.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'a' { 'b' } else { 'a' };
        let tampered: String = chars.into_iter().collect();

        assert!(
            unwrap_dek(&kek, &tampered).is_err(),
            "Authentication tag must reject a tampered key blob"
        );

        assert!(unwrap_dek(&kek, "not-hex").is_err());
        assert!(unwrap_dek(&kek, "abcd").is_err(), "Short blob must be rejected");
        Ok(())
    }

    /// Regression test for the data-loss bug: changing the master password used to
    /// derive a brand-new file key, which made every existing encrypted file
    /// permanently unreadable. With envelope encryption the DEK survives the change.
    #[test]
    fn test_password_change_keeps_files_decryptable() -> Result<(), Box<dyn std::error::Error>> {
        let input = Path::new("test_pwchange_input.bin");
        let encrypted = Path::new("test_pwchange_encrypted.vault");
        let decrypted = Path::new("test_pwchange_decrypted.bin");

        // --- Vault setup with the original password ---
        let old_kek = derive_key("old-master-password", "originalsaltvalue")?;
        let dek = generate_dek();
        let wrapped = wrap_dek(&old_kek, &dek)?;

        // Encrypt a file with the DEK
        create_test_file(input, 2 * 1024 * 1024)?;
        let original_data = fs::read(input)?;
        encrypt_file_streaming(&dek, input, encrypted)?;

        // --- User changes the master password (new salt, new KEK, same DEK) ---
        let recovered = unwrap_dek(&old_kek, &wrapped)?;
        let new_kek = derive_key("brand-new-password", "differentsaltvalue")?;
        let rewrapped = wrap_dek(&new_kek, &recovered)?;

        // --- Later session: log in with the new password only ---
        let session_kek = derive_key("brand-new-password", "differentsaltvalue")?;
        let session_dek = unwrap_dek(&session_kek, &rewrapped)?;

        decrypt_file_streaming(&session_dek, encrypted, decrypted)?;
        let decrypted_data = fs::read(decrypted)?;

        assert_eq!(
            original_data, decrypted_data,
            "File encrypted before the password change must still decrypt after it"
        );

        // The old password must no longer open the vault
        assert!(unwrap_dek(&old_kek, &rewrapped).is_err());

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;
        Ok(())
    }

    /// Legacy vaults have no wrapped DEK; the password-derived key *is* the file key.
    /// Adopting it as the DEK must keep old files readable.
    #[test]
    fn test_legacy_vault_migration() -> Result<(), Box<dyn std::error::Error>> {
        let input = Path::new("test_legacy_input.bin");
        let encrypted = Path::new("test_legacy_encrypted.vault");
        let decrypted = Path::new("test_legacy_decrypted.bin");

        // Old behaviour: file key derived straight from the password
        let legacy_key = derive_key("legacy-password", "legacysaltvalue")?;
        create_test_file(input, 1024 * 1024)?;
        let original_data = fs::read(input)?;
        encrypt_file_streaming(&legacy_key, input, encrypted)?;

        // Migration on next login: adopt the derived key as the DEK and wrap it
        let kek = derive_key("legacy-password", "legacysaltvalue")?;
        let wrapped = wrap_dek(&kek, &kek)?;
        let dek = unwrap_dek(&kek, &wrapped)?;

        decrypt_file_streaming(&dek, encrypted, decrypted)?;
        assert_eq!(original_data, fs::read(decrypted)?, "Legacy file must survive migration");

        fs::remove_file(input)?;
        fs::remove_file(encrypted)?;
        fs::remove_file(decrypted)?;
        Ok(())
    }

    #[test]
    fn test_chunk_nonce_derivation() {
        // Verify that different chunk indices produce different nonces
        let master = [0x42u8; MASTER_NONCE_LEN];

        let nonce0 = derive_chunk_nonce(&master, 0);
        let nonce1 = derive_chunk_nonce(&master, 1);
        let nonce2 = derive_chunk_nonce(&master, 2);

        // All nonces should be different
        assert_ne!(nonce0, nonce1);
        assert_ne!(nonce1, nonce2);
        assert_ne!(nonce0, nonce2);

        // Derive same index again should give same nonce
        let nonce0_again = derive_chunk_nonce(&master, 0);
        assert_eq!(nonce0, nonce0_again, "Same index should produce same nonce");
    }
}
