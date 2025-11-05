/// Prefetching Module for Optimized I/O
///
/// Implements read-ahead strategy to overlap I/O with computation.
/// Uses a background thread to read chunks ahead while the main thread
/// encrypts/decrypts, reducing wait time for I/O operations.

use std::fs::File;
use std::io::Read;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

/// Configuration for prefetching behavior
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// Number of chunks to prefetch ahead
    pub chunk_count: usize,
    /// Size of each chunk (bytes)
    pub chunk_size: usize,
    /// Enable prefetching (true) or disable (false)
    pub enabled: bool,
}

impl PrefetchConfig {
    /// Create prefetch config based on storage characteristics
    pub fn new(prefetch_chunks: usize, chunk_size: usize) -> Self {
        Self {
            chunk_count: prefetch_chunks,
            chunk_size,
            enabled: prefetch_chunks > 0,
        }
    }

    /// Disable prefetching
    pub fn disabled() -> Self {
        Self {
            chunk_count: 0,
            chunk_size: 0,
            enabled: false,
        }
    }
}

/// Prefetcher manages background read-ahead operations
pub struct Prefetcher {
    /// Configuration
    config: PrefetchConfig,
    /// Sender for requesting prefetch
    request_tx: Sender<PrefetchRequest>,
    /// Receiver for getting prefetched chunks
    data_rx: Receiver<PrefetchedChunk>,
    /// Background thread handle
    _thread: Option<thread::JoinHandle<()>>,
}

/// Request types for the prefetch thread
enum PrefetchRequest {
    /// Read and prefetch the specified chunk
    Read { file_path: String, chunk_idx: u64 },
    /// Stop prefetching and exit
    Stop,
}

/// A prefetched chunk with metadata
#[derive(Debug)]
pub struct PrefetchedChunk {
    /// Chunk index (order identifier)
    pub chunk_idx: u64,
    /// Actual data read
    pub data: Vec<u8>,
    /// Bytes actually read (may be less than chunk_size)
    pub bytes_read: usize,
    /// Any error that occurred
    pub error: Option<String>,
}

impl Prefetcher {
    /// Create a new prefetcher
    ///
    /// Spawns a background thread that listens for prefetch requests
    /// and reads chunks ahead of time.
    pub fn new(config: PrefetchConfig) -> Self {
        if !config.enabled {
            // Prefetching disabled - return a no-op prefetcher
            let (_tx, _request_rx) = channel::<PrefetchRequest>();
            let (_data_tx, data_rx) = channel::<PrefetchedChunk>();
            return Self {
                config,
                request_tx: _tx,
                data_rx,
                _thread: None,
            };
        }

        let (request_tx, request_rx) = channel::<PrefetchRequest>();
        let (data_tx, data_rx) = channel::<PrefetchedChunk>();

        let thread = thread::spawn(move || {
            let mut file: Option<File> = None;
            let mut current_path: String = String::new();

            while let Ok(request) = request_rx.recv() {
                match request {
                    PrefetchRequest::Read {
                        file_path,
                        chunk_idx,
                    } => {
                        // Open file if path changed
                        if file_path != current_path {
                            match File::open(&file_path) {
                                Ok(f) => {
                                    file = Some(f);
                                    current_path = file_path.clone();
                                }
                                Err(e) => {
                                    let _ = data_tx.send(PrefetchedChunk {
                                        chunk_idx,
                                        data: Vec::new(),
                                        bytes_read: 0,
                                        error: Some(format!("Failed to open file: {}", e)),
                                    });
                                    continue;
                                }
                            }
                        }

                        // Read chunk
                        if let Some(ref mut f) = file {
                            let chunk_size = 64 * 1024 * 1024; // Default 64MB
                            let mut buffer = vec![0u8; chunk_size];

                            match f.read(&mut buffer) {
                                Ok(bytes_read) => {
                                    buffer.truncate(bytes_read);
                                    let _ = data_tx.send(PrefetchedChunk {
                                        chunk_idx,
                                        data: buffer,
                                        bytes_read,
                                        error: None,
                                    });
                                }
                                Err(e) => {
                                    let _ = data_tx.send(PrefetchedChunk {
                                        chunk_idx,
                                        data: Vec::new(),
                                        bytes_read: 0,
                                        error: Some(format!("Read error: {}", e)),
                                    });
                                }
                            }
                        }
                    }
                    PrefetchRequest::Stop => break,
                }
            }
        });

        Self {
            config,
            request_tx,
            data_rx,
            _thread: Some(thread),
        }
    }

    /// Request prefetch of a chunk
    pub fn prefetch(&self, file_path: &str, chunk_idx: u64) {
        if !self.config.enabled {
            return;
        }

        let _ = self.request_tx.send(PrefetchRequest::Read {
            file_path: file_path.to_string(),
            chunk_idx,
        });
    }

    /// Get next prefetched chunk (blocks until available)
    pub fn get_next(&self) -> Option<PrefetchedChunk> {
        if !self.config.enabled {
            return None;
        }

        self.data_rx.recv().ok()
    }

    /// Try to get next prefetched chunk without blocking
    pub fn try_get_next(&self) -> Option<PrefetchedChunk> {
        if !self.config.enabled {
            return None;
        }

        self.data_rx.try_recv().ok()
    }

    /// Stop the prefetcher and wait for background thread
    pub fn stop(self) {
        if self.config.enabled {
            let _ = self.request_tx.send(PrefetchRequest::Stop);
            // Thread is joined when dropped
        }
    }
}

impl Drop for Prefetcher {
    fn drop(&mut self) {
        // Signal thread to stop if it exists
        if self.config.enabled {
            let _ = self.request_tx.send(PrefetchRequest::Stop);
        }
    }
}

/// Simple read-ahead queue for managing prefetched data
pub struct ReadAheadQueue {
    /// Buffered chunks waiting to be processed
    chunks: Arc<Mutex<Vec<PrefetchedChunk>>>,
    /// Prefetcher instance (kept alive for lifetime management)
    _prefetcher: Option<Prefetcher>,
}

impl ReadAheadQueue {
    /// Create a new read-ahead queue
    pub fn new(prefetch_config: PrefetchConfig) -> Self {
        Self {
            chunks: Arc::new(Mutex::new(Vec::new())),
            _prefetcher: if prefetch_config.enabled {
                Some(Prefetcher::new(prefetch_config))
            } else {
                None
            },
        }
    }

    /// Add a prefetched chunk to the queue
    pub fn enqueue(&self, chunk: PrefetchedChunk) {
        if let Ok(mut queue) = self.chunks.lock() {
            queue.push(chunk);
        }
    }

    /// Remove the next chunk from the queue
    pub fn dequeue(&self) -> Option<PrefetchedChunk> {
        if let Ok(mut queue) = self.chunks.lock() {
            if !queue.is_empty() {
                Some(queue.remove(0))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get number of chunks currently in queue
    pub fn len(&self) -> usize {
        if let Ok(queue) = self.chunks.lock() {
            queue.len()
        } else {
            0
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefetch_config_enabled() {
        let config = PrefetchConfig::new(4, 64 * 1024 * 1024);
        assert!(config.enabled);
        assert_eq!(config.chunk_count, 4);
    }

    #[test]
    fn test_prefetch_config_disabled() {
        let config = PrefetchConfig::disabled();
        assert!(!config.enabled);
        assert_eq!(config.chunk_count, 0);
    }

    #[test]
    fn test_read_ahead_queue_empty() {
        let config = PrefetchConfig::disabled();
        let queue = ReadAheadQueue::new(config);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_read_ahead_queue_enqueue_dequeue() {
        let config = PrefetchConfig::disabled();
        let queue = ReadAheadQueue::new(config);

        let chunk = PrefetchedChunk {
            chunk_idx: 0,
            data: vec![1, 2, 3, 4, 5],
            bytes_read: 5,
            error: None,
        };

        queue.enqueue(chunk);
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());

        let retrieved = queue.dequeue();
        assert!(retrieved.is_some());
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_prefetcher_disabled() {
        let config = PrefetchConfig::disabled();
        let prefetcher = Prefetcher::new(config);

        // Should not crash even with disabled prefetching
        prefetcher.prefetch("test.bin", 0);
        assert!(prefetcher.try_get_next().is_none());
    }
}
