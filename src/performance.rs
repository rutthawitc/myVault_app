/// Performance Configuration Module
///
/// This module detects system hardware capabilities and provides
/// optimal configuration for parallel encryption/decryption operations.

use std::fmt;

/// Performance configuration based on system hardware
///
/// Automatically detects:
/// - Available CPU cores
/// - Optimal thread count
/// - Chunk sizes for different file sizes
/// - Memory-mapping thresholds
/// - Storage type and characteristics
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Number of threads for parallel operations (total available - 1 for UI)
    pub thread_count: usize,

    /// Size of each chunk for encryption (bytes)
    pub chunk_size: usize,

    /// Files larger than this use memory mapping
    pub mmap_threshold: u64,

    /// Number of CPU cores detected on system
    pub cpu_cores: usize,

    /// Storage device information (SSD/HDD detection)
    pub storage_info: crate::storage::StorageInfo,
}

impl PerformanceConfig {
    /// Auto-detect optimal configuration based on system hardware
    ///
    /// This function:
    /// - Detects the number of CPU cores
    /// - Detects storage type (SSD, HDD, Network)
    /// - Reserves 1 core for UI thread
    /// - Sets chunk size to 64MB (optimal baseline)
    /// - Sets mmap threshold to 100MB
    /// - Detects storage characteristics for I/O optimization
    pub fn auto_detect() -> Self {
        let cpu_cores = num_cpus::get();
        let thread_count = (cpu_cores - 1).max(1);  // Leave 1 core for UI

        // Detect storage type and characteristics
        let storage_info = crate::storage::StorageInfo::detect(std::path::Path::new("."));

        Self {
            thread_count,
            chunk_size: 64 * 1024 * 1024,  // 64MB chunks
            mmap_threshold: 100 * 1024 * 1024,  // Use mmap for files > 100MB
            cpu_cores,
            storage_info,
        }
    }

    /// Determine optimal thread count for a specific file operation
    ///
    /// Returns the number of threads that should be used based on:
    /// - File size (small files: 1 thread, no overhead)
    /// - Available cores
    /// - Type of operation
    ///
    /// Examples:
    /// ```ignore
    /// let config = PerformanceConfig::auto_detect();
    /// config.threads_for_file(1024);           // 1 thread (overhead not worth it)
    /// config.threads_for_file(100_000_000);    // threads/2 (medium file)
    /// config.threads_for_file(1_000_000_000);  // all threads (large file)
    /// ```
    pub fn threads_for_file(&self, file_size: u64) -> usize {
        match file_size {
            // Small files: single-threaded (overhead not worth it)
            size if size < 10 * 1024 * 1024 => 1,

            // Medium files: half the available cores
            size if size < 100 * 1024 * 1024 => (self.thread_count / 2).max(1),

            // Large files: all available cores
            _ => self.thread_count,
        }
    }

    /// Determine if a file should use memory mapping
    ///
    /// Memory-mapped files are more efficient for large files as they:
    /// - Avoid explicit read() syscalls
    /// - Let OS handle paging
    /// - Reduce memory allocations
    pub fn should_use_mmap(&self, file_size: u64) -> bool {
        file_size > self.mmap_threshold
    }

    /// Get chunk size appropriate for file size
    ///
    /// Currently returns fixed 64MB chunks, but can be extended
    /// to vary based on file size or storage type detection
    pub fn get_chunk_size(&self, _file_size: u64) -> usize {
        self.chunk_size
    }

    /// Get recommended I/O buffer size
    ///
    /// Larger buffer = fewer syscalls but more memory
    /// Standard is 64MB, matching chunk_size
    pub fn get_io_buffer_size(&self) -> usize {
        self.chunk_size
    }
}

impl fmt::Display for PerformanceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PerformanceConfig {{\n  CPU cores: {}\n  Thread count: {}\n  Chunk size: {} MB\n  Mmap threshold: {} MB\n  Storage: {}\n}}",
            self.cpu_cores,
            self.thread_count,
            self.chunk_size / (1024 * 1024),
            self.mmap_threshold / (1024 * 1024),
            self.storage_info,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_detect() {
        let config = PerformanceConfig::auto_detect();
        assert!(config.thread_count > 0);
        assert!(config.cpu_cores > 0);
        assert_eq!(config.chunk_size, 64 * 1024 * 1024);
        assert_eq!(config.mmap_threshold, 100 * 1024 * 1024);
    }

    #[test]
    fn test_threads_for_small_file() {
        let config = PerformanceConfig::auto_detect();
        let threads = config.threads_for_file(1024);  // 1KB
        assert_eq!(threads, 1);
    }

    #[test]
    fn test_threads_for_medium_file() {
        let config = PerformanceConfig::auto_detect();
        let threads = config.threads_for_file(50 * 1024 * 1024);  // 50MB
        let expected = (config.thread_count / 2).max(1);
        assert_eq!(threads, expected);
    }

    #[test]
    fn test_threads_for_large_file() {
        let config = PerformanceConfig::auto_detect();
        let threads = config.threads_for_file(500 * 1024 * 1024);  // 500MB
        assert_eq!(threads, config.thread_count);
    }

    #[test]
    fn test_should_use_mmap() {
        let config = PerformanceConfig::auto_detect();
        assert!(!config.should_use_mmap(50 * 1024 * 1024));   // 50MB - no mmap
        assert!(config.should_use_mmap(150 * 1024 * 1024));   // 150MB - use mmap
    }

    #[test]
    fn test_display() {
        let config = PerformanceConfig::auto_detect();
        let display_str = format!("{}", config);
        assert!(display_str.contains("PerformanceConfig"));
        assert!(display_str.contains("CPU cores"));
        assert!(display_str.contains("Thread count"));
    }
}
