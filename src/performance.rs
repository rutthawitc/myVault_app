/// Performance Configuration Module
///
/// Detects CPU capacity and decides how many worker threads batch operations
/// may use.
use std::fmt;

/// Performance configuration derived from system hardware.
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Number of threads for parallel operations (cores available, minus 1 for UI).
    pub thread_count: usize,

    /// Number of CPU cores detected on the system.
    pub cpu_cores: usize,
}

impl PerformanceConfig {
    /// Auto-detect configuration based on the system's CPU count.
    ///
    /// Reserves one core for the UI thread so batch operations never starve it.
    pub fn auto_detect() -> Self {
        let cpu_cores = num_cpus::get();
        let thread_count = cpu_cores.saturating_sub(1).max(1);
        Self {
            thread_count,
            cpu_cores,
        }
    }
}

impl fmt::Display for PerformanceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PerformanceConfig {{ CPU cores: {}, Thread count: {} }}",
            self.cpu_cores, self.thread_count
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
        // At least one core is always reserved unless there is only one.
        assert!(config.thread_count <= config.cpu_cores);
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
