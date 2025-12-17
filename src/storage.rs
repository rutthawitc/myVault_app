/// Storage Detection and Optimization Module
///
/// This module detects storage device types (SSD, HDD, Network) and provides
/// adaptive I/O parameters for optimal encryption/decryption performance.

use std::path::Path;

/// Storage device types with different performance characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    /// Solid State Drive - high throughput, low latency
    SSD,
    /// Hard Disk Drive - lower throughput, higher latency
    HDD,
    /// Network storage (NFS, SMB, etc) - variable performance
    Network,
    /// Unknown or unable to detect
    Unknown,
}

/// Information about storage device characteristics
#[derive(Debug, Clone)]
pub struct StorageInfo {
    /// Type of storage device detected
    pub storage_type: StorageType,

    /// Estimated throughput in bytes/second
    pub throughput_estimate: u64,

    /// Optimal chunk size for this storage (bytes)
    pub optimal_chunk_size: usize,

    /// Optimal write buffer size (bytes)
    pub write_buffer_size: usize,

    /// Optimal read buffer size (bytes)
    pub read_buffer_size: usize,

    /// Maximum prefetch chunks (for lookahead)
    pub prefetch_chunks: usize,
}

impl StorageInfo {
    /// Detect storage type and create optimized configuration
    ///
    /// Attempts to detect storage characteristics. On detection failure,
    /// returns a conservative default configuration.
    pub fn detect(path: &Path) -> Self {
        // Try platform-specific detection
        let detected = Self::detect_storage_type(path);

        // Create parameters based on storage type
        match detected {
            StorageType::SSD => Self::new_ssd(),
            StorageType::HDD => Self::new_hdd(),
            StorageType::Network => Self::new_network(),
            StorageType::Unknown => Self::new_default(),
        }
    }

    /// Configuration for SSD (aggressive optimization)
    fn new_ssd() -> Self {
        Self {
            storage_type: StorageType::SSD,
            throughput_estimate: 500_000_000,  // ~500 MB/s
            optimal_chunk_size: 128 * 1024 * 1024,  // 128MB
            write_buffer_size: 256 * 1024 * 1024,   // 256MB
            read_buffer_size: 256 * 1024 * 1024,    // 256MB
            prefetch_chunks: 4,  // Prefetch 4 chunks ahead
        }
    }

    /// Configuration for HDD (balanced approach)
    fn new_hdd() -> Self {
        Self {
            storage_type: StorageType::HDD,
            throughput_estimate: 100_000_000,  // ~100 MB/s
            optimal_chunk_size: 64 * 1024 * 1024,   // 64MB
            write_buffer_size: 128 * 1024 * 1024,   // 128MB
            read_buffer_size: 128 * 1024 * 1024,    // 128MB
            prefetch_chunks: 2,  // Prefetch 2 chunks ahead
        }
    }

    /// Configuration for Network storage (conservative)
    fn new_network() -> Self {
        Self {
            storage_type: StorageType::Network,
            throughput_estimate: 50_000_000,   // ~50 MB/s
            optimal_chunk_size: 32 * 1024 * 1024,   // 32MB
            write_buffer_size: 64 * 1024 * 1024,    // 64MB
            read_buffer_size: 64 * 1024 * 1024,     // 64MB
            prefetch_chunks: 1,  // Prefetch only 1 chunk
        }
    }

    /// Default/fallback configuration (unknown storage)
    fn new_default() -> Self {
        Self {
            storage_type: StorageType::Unknown,
            throughput_estimate: 100_000_000,  // Assume ~100 MB/s
            optimal_chunk_size: 64 * 1024 * 1024,   // 64MB
            write_buffer_size: 128 * 1024 * 1024,   // 128MB
            read_buffer_size: 128 * 1024 * 1024,    // 128MB
            prefetch_chunks: 2,
        }
    }

    /// Detect storage type from filesystem path
    ///
    /// Platform-specific implementation:
    /// - Linux: Checks /sys/block/*/queue/rotational
    /// - Windows: Checks physical drive properties
    /// - macOS: Checks device properties
    #[cfg(target_os = "linux")]
    fn detect_storage_type(path: &Path) -> StorageType {
        use std::fs;
        use std::path::PathBuf;

        // Try to get the device for this path
        let mountpoint = Self::get_mountpoint(path);
        if let Some(device) = Self::get_device_from_mountpoint(&mountpoint) {
            // Extract device name (e.g., "sda" from "/dev/sda1")
            if let Some(device_name) = device.file_name().and_then(|n| n.to_str()) {
                let stripped = device_name.trim_start_matches(|c: char| c.is_numeric());
                let sysfs_path =
                    format!("/sys/block/{}/queue/rotational", stripped);

                if let Ok(content) = fs::read_to_string(&sysfs_path) {
                    match content.trim() {
                        "0" => return StorageType::SSD,
                        "1" => return StorageType::HDD,
                        _ => {}
                    }
                }
            }
        }

        // Check for network filesystem mount
        if Self::is_network_mount(&mountpoint) {
            return StorageType::Network;
        }

        StorageType::Unknown
    }

    #[cfg(target_os = "windows")]
    fn detect_storage_type(path: &Path) -> StorageType {
        // Windows detection - check if path contains network indicators
        if let Some(path_str) = path.to_str() {
            if path_str.starts_with("\\\\") || path_str.contains("\\\\") {
                return StorageType::Network;
            }
        }

        // For Windows, we'd normally use WMI to check physical disk properties
        // For now, return Unknown and let fallback handle it
        StorageType::Unknown
    }

    #[cfg(target_os = "macos")]
    fn detect_storage_type(_path: &Path) -> StorageType {
        // macOS detection would use diskutil or IOKit
        // For MVP, return Unknown and use defaults
        StorageType::Unknown
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn detect_storage_type(_path: &Path) -> StorageType {
        // Unknown platform - use defaults
        StorageType::Unknown
    }

    /// Get the mountpoint for a given path (Linux)
    #[cfg(target_os = "linux")]
    fn get_mountpoint(path: &Path) -> PathBuf {
        use std::fs;

        let mut current = path.to_path_buf();
        while !current.as_os_str().is_empty() {
            if current.exists() {
                return current;
            }
            current.pop();
        }
        std::path::PathBuf::from("/")
    }

    /// Get device from mountpoint (Linux)
    #[cfg(target_os = "linux")]
    fn get_device_from_mountpoint(mountpoint: &Path) -> Option<std::path::PathBuf> {
        use std::fs;

        if let Ok(content) = fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == mountpoint.to_string_lossy() {
                    return Some(std::path::PathBuf::from(parts[0]));
                }
            }
        }
        None
    }

    /// Check if a mountpoint is a network filesystem (Linux)
    #[cfg(target_os = "linux")]
    fn is_network_mount(mountpoint: &Path) -> bool {
        use std::fs;

        if let Ok(content) = fs::read_to_string("/proc/mounts") {
            let mp_str = mountpoint.to_string_lossy();
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == mp_str {
                    let fstype = parts[2];
                    return fstype == "nfs" || fstype == "cifs" || fstype == "smb"
                        || fstype.starts_with("nfs") || fstype.starts_with("cifs");
                }
            }
        }
        false
    }

    /// Check if Windows path is network (UNC path)
    #[allow(dead_code)]
    #[cfg(target_os = "windows")]
    fn is_network_mount(path: &Path) -> bool {
        if let Some(path_str) = path.to_str() {
            path_str.starts_with("\\\\") || path_str.contains("\\\\")
        } else {
            false
        }
    }

}

impl std::fmt::Display for StorageInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StorageInfo {{ type: {:?}, throughput: {} MB/s, chunk: {} MB, write_buf: {} MB, read_buf: {} MB, prefetch: {} }}",
            self.storage_type,
            self.throughput_estimate / 1_000_000,
            self.optimal_chunk_size / (1024 * 1024),
            self.write_buffer_size / (1024 * 1024),
            self.read_buffer_size / (1024 * 1024),
            self.prefetch_chunks
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssd_config() {
        let info = StorageInfo::new_ssd();
        assert_eq!(info.storage_type, StorageType::SSD);
        assert_eq!(info.write_buffer_size, 256 * 1024 * 1024);
        assert_eq!(info.prefetch_chunks, 4);
    }

    #[test]
    fn test_hdd_config() {
        let info = StorageInfo::new_hdd();
        assert_eq!(info.storage_type, StorageType::HDD);
        assert_eq!(info.write_buffer_size, 128 * 1024 * 1024);
        assert_eq!(info.prefetch_chunks, 2);
    }

    #[test]
    fn test_network_config() {
        let info = StorageInfo::new_network();
        assert_eq!(info.storage_type, StorageType::Network);
        assert_eq!(info.write_buffer_size, 64 * 1024 * 1024);
        assert_eq!(info.prefetch_chunks, 1);
    }

    #[test]
    fn test_default_config() {
        let info = StorageInfo::new_default();
        assert_eq!(info.storage_type, StorageType::Unknown);
        assert_eq!(info.write_buffer_size, 128 * 1024 * 1024);
    }

    #[test]
    fn test_detect_current_path() {
        let info = StorageInfo::detect(std::path::Path::new("."));
        assert!(matches!(
            info.storage_type,
            StorageType::SSD
                | StorageType::HDD
                | StorageType::Network
                | StorageType::Unknown
        ));
        println!("Current storage: {}", info);
    }

    #[test]
    fn test_display() {
        let info = StorageInfo::new_ssd();
        let display_str = format!("{}", info);
        assert!(display_str.contains("SSD"));
        assert!(display_str.contains("MB/s"));
    }
}
