/// Throughput Monitoring Module
///
/// Tracks I/O and encryption throughput to enable adaptive optimization.
/// Measures bytes processed per second and adjusts parallelism accordingly.

use std::time::{Instant, Duration};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Tracks throughput metrics
#[derive(Clone)]
pub struct ThroughputMonitor {
    /// Bytes processed so far
    bytes_processed: Arc<AtomicU64>,
    /// Start time of measurement period
    start_time: Instant,
    /// Last calculated throughput (bytes/sec)
    last_throughput: Arc<AtomicU64>,
}

impl ThroughputMonitor {
    /// Create a new throughput monitor
    pub fn new() -> Self {
        Self {
            bytes_processed: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            last_throughput: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record bytes processed
    pub fn record(&self, bytes: u64) {
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get current throughput in bytes per second
    pub fn get_throughput(&self) -> u64 {
        let elapsed = self.start_time.elapsed();
        if elapsed.as_millis() == 0 {
            return 0;
        }

        let bytes = self.bytes_processed.load(Ordering::Relaxed);
        let throughput = (bytes as u128 * 1000) / elapsed.as_millis();
        let result = throughput as u64;

        self.last_throughput.store(result, Ordering::Relaxed);
        result
    }

    /// Get throughput in MB/s
    pub fn get_throughput_mb_s(&self) -> f64 {
        self.get_throughput() as f64 / (1024.0 * 1024.0)
    }

    /// Get total bytes processed
    pub fn get_total_bytes(&self) -> u64 {
        self.bytes_processed.load(Ordering::Relaxed)
    }

    /// Get elapsed time
    pub fn get_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Reset counters for next measurement period
    pub fn reset(&mut self) {
        self.bytes_processed.store(0, Ordering::Relaxed);
        self.start_time = Instant::now();
        self.last_throughput.store(0, Ordering::Relaxed);
    }
}

impl Default for ThroughputMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive parallelism controller
pub struct AdaptiveController {
    /// Target throughput in bytes/sec
    target_throughput: u64,
    /// Current thread count
    current_threads: usize,
    /// Minimum threads to use
    min_threads: usize,
    /// Maximum threads to use
    max_threads: usize,
    /// Throughput monitor
    monitor: ThroughputMonitor,
}

impl AdaptiveController {
    /// Create a new adaptive controller
    pub fn new(
        target_throughput: u64,
        current_threads: usize,
        max_threads: usize,
    ) -> Self {
        let min_threads = (max_threads / 4).max(1); // Use at least 1/4 of max

        Self {
            target_throughput,
            current_threads,
            min_threads,
            max_threads,
            monitor: ThroughputMonitor::new(),
        }
    }

    /// Record bytes processed
    pub fn record(&self, bytes: u64) {
        self.monitor.record(bytes);
    }

    /// Analyze throughput and return recommended thread adjustment
    ///
    /// Returns:
    /// - Positive: increase threads by this amount
    /// - Negative: decrease threads by this amount (absolute value)
    /// - 0: no change needed
    pub fn analyze(&self) -> i32 {
        let current = self.monitor.get_throughput();

        // If below 80% of target, we're I/O bottlenecked - reduce threads
        if current < (self.target_throughput * 80) / 100 {
            if self.current_threads > self.min_threads {
                return -1; // Reduce by 1 thread
            }
        }

        // If above 120% of target, we have headroom - increase threads
        if current > (self.target_throughput * 120) / 100 {
            if self.current_threads < self.max_threads {
                return 1; // Increase by 1 thread
            }
        }

        0 // No change needed
    }

    /// Get current thread recommendation
    pub fn get_recommended_threads(&self) -> usize {
        match self.analyze() {
            adjustment if adjustment > 0 => {
                (self.current_threads + adjustment as usize).min(self.max_threads)
            }
            adjustment if adjustment < 0 => {
                (self.current_threads as i32 + adjustment).max(self.min_threads as i32) as usize
            }
            _ => self.current_threads,
        }
    }

    /// Update current thread count
    pub fn set_current_threads(&mut self, threads: usize) {
        self.current_threads = threads;
    }

    /// Get throughput in MB/s
    pub fn get_throughput_mb_s(&self) -> f64 {
        self.monitor.get_throughput_mb_s()
    }

    /// Reset monitoring for next period
    pub fn reset(&mut self) {
        let mut monitor = ThroughputMonitor::new();
        std::mem::swap(&mut self.monitor, &mut monitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_throughput_monitor_creation() {
        let monitor = ThroughputMonitor::new();
        assert_eq!(monitor.get_total_bytes(), 0);
    }

    #[test]
    fn test_throughput_monitor_record() {
        let monitor = ThroughputMonitor::new();
        monitor.record(1024);
        assert_eq!(monitor.get_total_bytes(), 1024);

        monitor.record(2048);
        assert_eq!(monitor.get_total_bytes(), 3072);
    }

    #[test]
    fn test_throughput_calculation() {
        let monitor = ThroughputMonitor::new();
        monitor.record(1024 * 1024); // 1MB

        thread::sleep(Duration::from_millis(100));

        let throughput = monitor.get_throughput();
        // After 100ms, 1MB should give us roughly 10MB/s
        // Allow 50% variance due to timing
        assert!(throughput > 5_000_000 && throughput < 20_000_000);
    }

    #[test]
    fn test_adaptive_controller_no_change() {
        let mut controller = AdaptiveController::new(100_000_000, 4, 8);

        // Simulate being at target throughput
        controller.monitor.record(100_000_000);
        thread::sleep(Duration::from_millis(100));

        let adjustment = controller.analyze();
        // Should be close to 0 (within tolerance)
        assert!(adjustment >= -1 && adjustment <= 1);
    }

    #[test]
    fn test_adaptive_controller_min_threads() {
        let controller = AdaptiveController::new(1_000_000_000, 1, 8);
        // At min threads, should stay at min
        assert_eq!(controller.current_threads, 1);
    }

    #[test]
    fn test_adaptive_controller_max_threads() {
        let controller = AdaptiveController::new(100_000_000, 8, 8);
        // At max threads, should stay at max
        assert_eq!(controller.current_threads, 8);
    }

    #[test]
    fn test_throughput_mb_s() {
        let monitor = ThroughputMonitor::new();
        monitor.record(64 * 1024 * 1024); // 64MB

        thread::sleep(Duration::from_millis(100));

        let throughput_mb_s = monitor.get_throughput_mb_s();
        // Should be roughly 640 MB/s (64MB in 100ms)
        // Allow 50% variance
        assert!(throughput_mb_s > 320.0 && throughput_mb_s < 1280.0);
    }
}
