/// Progress Tracking Module for UI Responsiveness
///
/// Provides real-time progress updates for encryption/decryption operations.
/// Allows UI to display progress bars and remain responsive during long operations.

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Progress state for a single operation
#[derive(Debug, Clone)]
pub struct ProgressState {
    /// Total bytes to process
    pub total_bytes: u64,
    /// Bytes processed so far
    pub bytes_processed: u64,
    /// Operation start time
    pub start_time: Option<Instant>,
    /// Whether operation is cancelled
    pub cancelled: bool,
    /// Current status message
    pub status: String,
}

impl ProgressState {
    /// Create a new progress state
    pub fn new(total_bytes: u64) -> Self {
        Self {
            total_bytes,
            bytes_processed: 0,
            start_time: Some(Instant::now()),
            cancelled: false,
            status: "Starting...".to_string(),
        }
    }

    /// Get progress as percentage (0-100)
    pub fn progress_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            100.0
        } else {
            ((self.bytes_processed as f64 / self.total_bytes as f64) * 100.0) as f32
        }
    }

    /// Get estimated time remaining in seconds
    pub fn estimated_time_remaining(&self) -> Option<f64> {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f64();
            if elapsed > 0.0 && self.bytes_processed > 0 {
                let total_estimated = (self.total_bytes as f64 / self.bytes_processed as f64) * elapsed;
                return Some((total_estimated - elapsed).max(0.0));
            }
        }
        None
    }

    /// Get current throughput in MB/s
    pub fn current_throughput_mb_s(&self) -> Option<f64> {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let throughput = self.bytes_processed as f64 / elapsed / (1024.0 * 1024.0);
                return Some(throughput);
            }
        }
        None
    }

    /// Check if operation is complete
    pub fn is_complete(&self) -> bool {
        self.bytes_processed >= self.total_bytes && self.total_bytes > 0
    }
}

/// Thread-safe progress tracker using atomic operations
pub struct ProgressTracker {
    /// Total bytes to process
    total_bytes: Arc<AtomicU64>,
    /// Bytes processed so far
    bytes_processed: Arc<AtomicU64>,
    /// Cancellation flag
    cancelled: Arc<AtomicBool>,
    /// Start time for throughput calculation
    start_time: Instant,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(total_bytes: u64) -> Self {
        Self {
            total_bytes: Arc::new(AtomicU64::new(total_bytes)),
            bytes_processed: Arc::new(AtomicU64::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
        }
    }

    /// Report progress (bytes processed)
    pub fn report_progress(&self, bytes: u64) {
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Set progress to exact value
    pub fn set_progress(&self, bytes: u64) {
        self.bytes_processed.store(bytes, Ordering::Relaxed);
    }

    /// Request cancellation
    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Get current progress state snapshot
    pub fn get_state(&self) -> ProgressState {
        let bytes_proc = self.bytes_processed.load(Ordering::Relaxed);
        let total = self.total_bytes.load(Ordering::Relaxed);

        ProgressState {
            total_bytes: total,
            bytes_processed: bytes_proc,
            start_time: Some(self.start_time),
            cancelled: self.is_cancelled(),
            status: format!(
                "{:.1}% ({}/{})",
                (bytes_proc as f64 / total as f64 * 100.0).max(0.0),
                bytes_proc / (1024 * 1024),
                total / (1024 * 1024)
            ),
        }
    }

    /// Get progress percentage
    pub fn progress_percent(&self) -> f32 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        let processed = self.bytes_processed.load(Ordering::Relaxed);
        ((processed as f64 / total as f64) * 100.0) as f32
    }

    /// Get current throughput in MB/s
    pub fn get_throughput_mb_s(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let bytes = self.bytes_processed.load(Ordering::Relaxed);
            return bytes as f64 / elapsed / (1024.0 * 1024.0);
        }
        0.0
    }

    /// Get estimated time remaining in seconds
    pub fn get_eta_seconds(&self) -> Option<f64> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let bytes_proc = self.bytes_processed.load(Ordering::Relaxed);

        if elapsed > 0.0 && bytes_proc > 0 {
            let total = self.total_bytes.load(Ordering::Relaxed);
            let total_estimated = (total as f64 / bytes_proc as f64) * elapsed;
            return Some((total_estimated - elapsed).max(0.0));
        }
        None
    }

    /// Get total elapsed time in seconds
    pub fn get_elapsed_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Clone the tracker for use in other threads
    pub fn clone_for_thread(&self) -> Self {
        Self {
            total_bytes: Arc::clone(&self.total_bytes),
            bytes_processed: Arc::clone(&self.bytes_processed),
            cancelled: Arc::clone(&self.cancelled),
            start_time: self.start_time,
        }
    }
}

impl Clone for ProgressTracker {
    fn clone(&self) -> Self {
        self.clone_for_thread()
    }
}

/// Manages progress for multiple concurrent operations
pub struct ProgressManager {
    /// Trackers for active operations
    trackers: Vec<ProgressTracker>,
}

impl ProgressManager {
    /// Create a new progress manager
    pub fn new() -> Self {
        Self {
            trackers: Vec::new(),
        }
    }

    /// Add a new operation tracker
    pub fn add_operation(&mut self, total_bytes: u64) -> ProgressTracker {
        let tracker = ProgressTracker::new(total_bytes);
        self.trackers.push(tracker.clone());
        tracker
    }

    /// Get overall progress across all operations
    pub fn get_overall_progress(&self) -> f32 {
        if self.trackers.is_empty() {
            return 100.0;
        }

        let total_bytes: u64 = self.trackers.iter()
            .map(|t| t.total_bytes.load(Ordering::Relaxed))
            .sum();

        if total_bytes == 0 {
            return 100.0;
        }

        let processed_bytes: u64 = self.trackers.iter()
            .map(|t| t.bytes_processed.load(Ordering::Relaxed))
            .sum();

        ((processed_bytes as f64 / total_bytes as f64) * 100.0) as f32
    }

    /// Cancel all operations
    pub fn cancel_all(&self) {
        for tracker in &self.trackers {
            tracker.request_cancel();
        }
    }

    /// Clear completed operations
    pub fn clear_completed(&mut self) {
        self.trackers.retain(|t| !t.is_cancelled() && !t.get_state().is_complete());
    }
}

impl Default for ProgressManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_progress_state_creation() {
        let state = ProgressState::new(1024 * 1024); // 1MB
        assert_eq!(state.total_bytes, 1024 * 1024);
        assert_eq!(state.bytes_processed, 0);
        assert!(!state.cancelled);
    }

    #[test]
    fn test_progress_percent() {
        let mut state = ProgressState::new(1000);
        state.bytes_processed = 500;
        assert_eq!(state.progress_percent(), 50.0);

        state.bytes_processed = 1000;
        assert_eq!(state.progress_percent(), 100.0);
    }

    #[test]
    fn test_progress_tracker_creation() {
        let tracker = ProgressTracker::new(1024 * 1024);
        assert_eq!(tracker.progress_percent(), 0.0);
        assert!(!tracker.is_cancelled());
    }

    #[test]
    fn test_progress_tracker_report() {
        let tracker = ProgressTracker::new(1024 * 1024);
        tracker.report_progress(512 * 1024);
        assert!((tracker.progress_percent() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_progress_tracker_set_progress() {
        let tracker = ProgressTracker::new(1000);
        tracker.set_progress(750);
        let state = tracker.get_state();
        assert_eq!(state.bytes_processed, 750);
    }

    #[test]
    fn test_progress_tracker_cancellation() {
        let tracker = ProgressTracker::new(1024 * 1024);
        assert!(!tracker.is_cancelled());

        tracker.request_cancel();
        assert!(tracker.is_cancelled());
    }

    #[test]
    fn test_progress_tracker_throughput() {
        let tracker = ProgressTracker::new(64 * 1024 * 1024); // 64MB
        tracker.report_progress(64 * 1024 * 1024);

        thread::sleep(Duration::from_millis(100));

        let throughput = tracker.get_throughput_mb_s();
        // Should be roughly 640 MB/s (64MB in 100ms)
        // Allow 50% variance
        assert!(throughput > 320.0 && throughput < 1280.0);
    }

    #[test]
    fn test_progress_tracker_eta() {
        let tracker = ProgressTracker::new(1000);
        tracker.report_progress(500);

        thread::sleep(Duration::from_millis(100));

        if let Some(eta) = tracker.get_eta_seconds() {
            // ETA should be roughly 100ms
            // Allow 100ms variance due to timing
            assert!(eta >= 0.0 && eta < 1.0);
        }
    }

    #[test]
    fn test_progress_manager_creation() {
        let manager = ProgressManager::new();
        assert_eq!(manager.trackers.len(), 0);
    }

    #[test]
    fn test_progress_manager_add_operation() {
        let mut manager = ProgressManager::new();
        let _tracker = manager.add_operation(1024 * 1024);
        assert_eq!(manager.trackers.len(), 1);
    }

    #[test]
    fn test_progress_manager_overall_progress() {
        let mut manager = ProgressManager::new();
        let t1 = manager.add_operation(1000);
        let t2 = manager.add_operation(1000);

        t1.report_progress(500);
        t2.report_progress(1000);

        let overall = manager.get_overall_progress();
        assert!((overall - 75.0).abs() < 1.0); // 75%
    }

    #[test]
    fn test_progress_tracker_clone() {
        let tracker1 = ProgressTracker::new(1024 * 1024);
        let tracker2 = tracker1.clone();

        tracker1.report_progress(512 * 1024);
        // Both should see the same progress
        assert!((tracker2.progress_percent() - 50.0).abs() < 0.1);
    }
}
