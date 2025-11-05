#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod crypto;
mod model;
mod platform;
mod performance;
mod storage;
mod prefetch;
mod throughput;
mod progress;

pub use performance::PerformanceConfig;
pub use storage::{StorageInfo, StorageType};
pub use prefetch::{Prefetcher, PrefetchConfig, PrefetchedChunk, ReadAheadQueue};
pub use throughput::{ThroughputMonitor, AdaptiveController};
pub use progress::{ProgressTracker, ProgressState, ProgressManager};

use eframe::egui;
use zeroize::Zeroize;
use model::{ItemType, VaultItem};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::collections::{VecDeque, HashSet};
use std::time::Instant;

fn main() -> eframe::Result<()> {
    let mut options = eframe::NativeOptions::default();

    // Set window icon with a simple vault icon (lock emoji-based)
    options.viewport.icon = Some(std::sync::Arc::new(create_vault_icon()));

    eframe::run_native(
        "My Vault App",
        options,
        Box::new(|_cc| Box::new(MyVaultApp::new())),
    )
}

/// Create a simple vault icon (lock symbol in pixels)
fn create_vault_icon() -> egui::IconData {
    // Create a 256x256 lock icon
    let mut pixels = vec![0u8; 256 * 256 * 4]; // RGBA format

    // Draw a simple lock shape (dark blue/purple lock on transparent background)
    let lock_color = [25, 118, 210, 255]; // Blue lock color (RGBA)

    // Lock body (rectangle) - centered at 128,128
    for y in 80..200 {
        for x in 80..176 {
            let idx = (y * 256 + x) * 4;
            pixels[idx..idx+4].copy_from_slice(&lock_color);
        }
    }

    // Lock shackle (top curved part)
    for y in 40..100 {
        for x in 100..156 {
            let dx = (x as i32 - 128).abs() as f32;
            let dy = (y as i32 - 70) as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 26.0 && dist > 16.0 {
                let idx = (y * 256 + x) * 4;
                pixels[idx..idx+4].copy_from_slice(&lock_color);
            }
        }
    }

    // Keyhole (small circle)
    for y in 130..160 {
        for x in 118..138 {
            let dx = (x as i32 - 128) as f32;
            let dy = (y as i32 - 145) as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 8.0 {
                let idx = (y * 256 + x) * 4;
                pixels[idx..idx+4].copy_from_slice(&[255, 255, 255, 255]); // White keyhole
            }
        }
    }

    egui::IconData {
        width: 256,
        height: 256,
        rgba: pixels,
    }
}

struct MyVaultApp {
    items: Vec<VaultItem>,
    selected: HashSet<usize>,  // Multi-select: set of selected item indices
    last_selected: Option<usize>,  // Track last selected index for shift+click range selection
    status_message: String,
    master_password_hash: Option<String>,
    salt: Option<String>,
    authenticated: bool,
    show_password_dialog: bool,
    temp_password: String,
    temp_password_confirm: String,
    encryption_key: Option<[u8; 32]>,
    confirm_action: Option<ConfirmAction>,
    current_op: Option<BatchOp>,
    op_result_rxs: Vec<Receiver<(PathBuf, bool, Option<String>)>>,  // Multiple background thread receivers for parallel processing (path, success, optional_error_msg)
    show_error_report: bool,
    last_error_report: Vec<(PathBuf, String)>,
    perf_config: PerformanceConfig,  // Dynamic performance configuration based on CPU cores
    show_change_password_dialog: bool,
    current_password: String,
    new_password: String,
    new_password_confirm: String,
    dark_mode: bool,  // Phase 1: Dark mode toggle
    // Phase 2: UX Improvements
    search_filter: String,
    recent_files: Vec<PathBuf>,
    sort_by: SortField,
    sort_ascending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortField {
    Name,
    Status,
    Size,
}

impl MyVaultApp {
    fn new() -> Self {
        let mut app = Self {
            items: Vec::new(),
            selected: HashSet::new(),
            last_selected: None,
            status_message: String::new(),
            master_password_hash: None,
            salt: None,
            authenticated: false,
            show_password_dialog: false,
            temp_password: String::new(),
            temp_password_confirm: String::new(),
            encryption_key: None,
            confirm_action: None,
            current_op: None,
            op_result_rxs: Vec::new(),
            show_error_report: false,
            last_error_report: Vec::new(),
            perf_config: PerformanceConfig::auto_detect(),  // Auto-detect optimal thread count
            show_change_password_dialog: false,
            current_password: String::new(),
            new_password: String::new(),
            new_password_confirm: String::new(),
            dark_mode: false,  // Default to light mode
            // Phase 2: UX Improvements
            search_filter: String::new(),
            recent_files: Vec::new(),
            sort_by: SortField::Name,
            sort_ascending: true,
        };
        app.load_from_config();
        app
    }

    fn perform_overwrite_lock(&mut self, src: &Path, dst: &Path) -> Result<(), String> {
        let key = self.encryption_key.as_ref().ok_or("Not authenticated")?;
        let data = std::fs::read(src).map_err(|e| e.to_string())?;
        let blob = crate::crypto::encrypt_blob(key, &data)?;
        std::fs::write(dst, blob).map_err(|e| e.to_string())?;
        let _ = crate::platform::hide(dst);
        std::fs::remove_file(src).map_err(|e| e.to_string())?;
        // Update all matching items
        for item in self.items.iter_mut() {
            if item.original_path == src {
                item.encrypted_path = Some(dst.to_path_buf());
                item.is_locked = true;
            }
        }
        self.save_config();
        self.status_message = "Locked file (overwritten)".to_string();
        Ok(())
    }

    fn perform_overwrite_unlock(&mut self, src_enc: &Path, dst: &Path) -> Result<(), String> {
        let key = self.encryption_key.as_ref().ok_or("Not authenticated")?;
        let _ = crate::platform::unhide(src_enc);
        let data = std::fs::read(src_enc).map_err(|e| e.to_string())?;
        let plain = crate::crypto::decrypt_blob(key, &data)?;
        std::fs::write(dst, plain).map_err(|e| e.to_string())?;
        std::fs::remove_file(src_enc).map_err(|e| e.to_string())?;
        // Update all matching items
        for item in self.items.iter_mut() {
            let expected_enc = item.encrypted_path.clone().unwrap_or_else(|| Self::encrypted_path_for(&item.original_path));
            if expected_enc == src_enc {
                item.encrypted_path = None;
                item.is_locked = false;
                item.original_path = dst.to_path_buf();
            }
        }
        self.save_config();
        self.status_message = "Unlocked file (overwritten)".to_string();
        Ok(())
    }

    fn load_from_config(&mut self) {
        match config::load_config() {
            Ok(cfg) => {
                self.master_password_hash = cfg.master_password_hash;
                self.salt = cfg.salt;
                self.items = cfg.vault_items.iter().map(|c| c.into()).collect();
                self.status_message = "Loaded configuration".to_string();
            }
            Err(e) => {
                self.status_message = format!("Failed to load config: {}", e);
            }
        }
    }

    fn save_config(&mut self) {
        if let Err(e) = config::save_config(&self.items, self.master_password_hash.as_deref(), self.salt.as_deref()) {
            self.status_message = format!("Failed to save config: {}", e);
        }
    }

    fn add_path(&mut self, path: PathBuf, item_type: ItemType) {
        let item = VaultItem {
            original_path: path,
            encrypted_path: None,
            is_locked: false,
            item_type,
        };
        self.items.push(item);
        let idx = self.items.len() - 1;
        self.selected.insert(idx);  // Select the newly added item
        self.status_message = "Added item".to_string();
        self.save_config();
    }

    fn remove_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }

        // Collect indices to remove (in reverse order to avoid index shifting issues)
        let mut indices: Vec<_> = self.selected.iter().copied().collect();
        indices.sort_by(|a, b| b.cmp(a));  // Sort descending

        for i in indices {
            if i < self.items.len() {
                self.items.remove(i);
            }
        }

        self.selected.clear();
        self.status_message = format!("Removed {} items", self.selected.len());
        self.save_config();
    }

    fn scan_locked_files(&mut self, folder: &Path) {
        // Recursively scan a folder for encrypted files and add them to the vault
        let mut found_count = 0;

        for entry in WalkDir::new(folder)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let file_path = entry.path();
            // Check if it's a MyVault encrypted file
            if crate::crypto::is_encrypted_file(file_path) {
                // Get original filename by removing .vault.encrypted suffix
                if let Some(original) = Self::original_path_for(file_path) {
                    // Check if this file is already in the vault
                    let already_added = self.items.iter().any(|item| {
                        item.encrypted_path.as_ref().map(|p| p == file_path).unwrap_or(false)
                    });

                    if !already_added {
                        // Add as locked file
                        let item = VaultItem {
                            original_path: original,
                            encrypted_path: Some(file_path.to_path_buf()),
                            is_locked: true,
                            item_type: ItemType::File,
                        };
                        self.items.push(item);
                        found_count += 1;
                    }
                }
            }
        }

        if found_count > 0 {
            self.save_config();
            self.status_message = format!("Found and added {} locked files", found_count);
        } else {
            self.status_message = "No locked files found in folder".to_string();
        }
    }

    fn encrypted_suffix() -> &'static str {
        ".vault.encrypted"
    }

    #[cfg_attr(target_os = "windows", allow(unused_mut))]
    fn encrypted_path_for(path: &Path) -> PathBuf {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let mut new_name = format!("{}{}", name, Self::encrypted_suffix());
        #[cfg(not(target_os = "windows"))]
        {
            // Prefix a dot to hide on Unix. This always adds one dot; if the
            // original already started with a dot, the encrypted name will start
            // with two dots and we will remove exactly one on unlock.
            new_name = format!(".{}", new_name);
        }
        path.with_file_name(new_name)
    }

    #[cfg_attr(target_os = "windows", allow(unused_mut))]
    fn original_path_for(encrypted: &Path) -> Option<PathBuf> {
        let name = encrypted.file_name()?.to_string_lossy();
        let suf = Self::encrypted_suffix();
        if !name.ends_with(suf) {
            return None;
        }
        let mut trimmed = name[..name.len() - suf.len()].to_string();
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(stripped) = trimmed.strip_prefix('.') {
                trimmed = stripped.to_string();
            }
        }
        Some(encrypted.with_file_name(trimmed))
    }

    fn lock_file(&mut self, path: &Path, interactive: bool) -> Result<PathBuf, String> {
        let key = self.encryption_key.as_ref().ok_or("Not authenticated")?;
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let blob = crate::crypto::encrypt_blob(key, &data)?;
        let out = Self::encrypted_path_for(path);
        if out.exists() {
            if interactive {
                self.status_message = format!("Encrypted file exists: {}", out.display());
                self.confirm_action = Some(ConfirmAction::OverwriteLock { src: path.to_path_buf(), dst: out.clone() });
            }
            return Err("Encrypted file already exists".to_string());
        }
        std::fs::write(&out, blob).map_err(|e| e.to_string())?;
        let _ = crate::platform::hide(&out);
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
        Ok(out)
    }

    fn unlock_file(&mut self, enc_path: &Path, interactive: bool) -> Result<PathBuf, String> {
        let key = self.encryption_key.as_ref().ok_or("Not authenticated")?;
        let _ = crate::platform::unhide(enc_path);
        let data = std::fs::read(enc_path).map_err(|e| e.to_string())?;
        let plain = crate::crypto::decrypt_blob(key, &data)?;
        let out = Self::original_path_for(enc_path).ok_or("Invalid encrypted filename")?;
        if out.exists() {
            if interactive {
                self.status_message = format!("Original file exists: {}", out.display());
                self.confirm_action = Some(ConfirmAction::OverwriteUnlock { src: enc_path.to_path_buf(), dst: out.clone() });
            }
            return Err("Original file already exists".to_string());
        }
        std::fs::write(&out, plain).map_err(|e| e.to_string())?;
        std::fs::remove_file(enc_path).map_err(|e| e.to_string())?;
        Ok(out)
    }

    fn lock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        if self.encryption_key.is_none() {
            self.status_message = "Not authenticated".to_string();
            return;
        }

        // Collect all selected indices
        let mut selected_indices: Vec<_> = self.selected.iter().copied().collect();
        selected_indices.sort();

        // Process each selected item and collect files/folders
        let mut all_files = VecDeque::new();
        let suffix = Self::encrypted_suffix().to_string();

        for idx in selected_indices.iter() {
            match self.items[*idx].item_type {
                ItemType::File => {
                    let path = self.items[*idx].original_path.clone();
                    all_files.push_back(path);
                }
                ItemType::Folder => {
                    let folder = self.items[*idx].original_path.clone();
                    // Collect all files in folder
                    for entry in WalkDir::new(&folder)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| e.file_type().is_file())
                    {
                        let p = entry.into_path();
                        let skip = p.file_name().map(|n| n.to_string_lossy().ends_with(&suffix)).unwrap_or(false);
                        if !skip {
                            all_files.push_back(p);
                        }
                    }
                }
            }
        }

        if all_files.is_empty() {
            self.status_message = "No items to lock".to_string();
            return;
        }

        // Start batch operation with all collected files
        self.current_op = Some(BatchOp {
            kind: BatchOpKind::LockFolder,
            queue: all_files,
            rx: None,
            scanning_done: true,
            processed: 0,
            failures: 0,
            item_index: *selected_indices.first().unwrap(),
            affected_items: selected_indices.clone(),
            error_details: Vec::new(),
            start_time: Instant::now(),
        });

        let file_count = self.current_op.as_ref().unwrap().queue.len();
        self.status_message = format!("Starting lock: {} files from {} items...", file_count, selected_indices.len());
    }

    fn unlock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        if self.encryption_key.is_none() {
            self.status_message = "Not authenticated".to_string();
            return;
        }

        // Collect all selected indices
        let mut selected_indices: Vec<_> = self.selected.iter().copied().collect();
        selected_indices.sort();

        // Process each selected item and collect encrypted files/folders
        let mut all_files = VecDeque::new();
        let suffix = Self::encrypted_suffix().to_string();

        for idx in selected_indices.iter() {
            match self.items[*idx].item_type {
                ItemType::File => {
                    let enc = match self.items[*idx].encrypted_path.clone() {
                        Some(p) => p,
                        None => Self::encrypted_path_for(&self.items[*idx].original_path),
                    };
                    all_files.push_back(enc);
                }
                ItemType::Folder => {
                    let folder = self.items[*idx].original_path.clone();
                    // Collect all encrypted files in folder
                    for entry in WalkDir::new(&folder)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| e.file_type().is_file())
                    {
                        let p = entry.into_path();
                        let is_encrypted = p.file_name().map(|n| n.to_string_lossy().ends_with(&suffix)).unwrap_or(false);
                        if is_encrypted {
                            all_files.push_back(p);
                        }
                    }
                }
            }
        }

        if all_files.is_empty() {
            self.status_message = "No locked items to unlock".to_string();
            return;
        }

        // Start batch operation with all collected encrypted files
        self.current_op = Some(BatchOp {
            kind: BatchOpKind::UnlockFolder,
            queue: all_files,
            rx: None,
            scanning_done: true,
            processed: 0,
            failures: 0,
            item_index: *selected_indices.first().unwrap(),
            affected_items: selected_indices.clone(),
            error_details: Vec::new(),
            start_time: Instant::now(),
        });

        let file_count = self.current_op.as_ref().unwrap().queue.len();
        self.status_message = format!("Starting unlock: {} files from {} items...", file_count, selected_indices.len());
    }
}

impl eframe::App for MyVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Phase 1: Apply dark mode theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Phase 2: Keyboard shortcuts
        let busy = self.current_op.is_some();
        if !busy && self.authenticated && !self.show_password_dialog && !self.show_change_password_dialog {
            ctx.input(|i| {
                // Ctrl+A: Select all
                if i.modifiers.ctrl && i.key_pressed(egui::Key::A) {
                    self.selected.clear();
                    for idx in 0..self.items.len() {
                        self.selected.insert(idx);
                    }
                }

                // Ctrl+L: Lock selected files
                if i.modifiers.ctrl && i.key_pressed(egui::Key::L) {
                    let has_selection = !self.selected.is_empty();
                    let some_selected_unlocked = self.selected.iter()
                        .any(|&idx| self.items.get(idx).map(|it| !it.is_locked).unwrap_or(false));
                    if has_selection && some_selected_unlocked {
                        self.confirm_action = Some(ConfirmAction::Lock);
                    }
                }

                // Ctrl+U: Unlock selected files
                if i.modifiers.ctrl && i.key_pressed(egui::Key::U) {
                    let has_selection = !self.selected.is_empty();
                    let all_selected_locked = !self.selected.is_empty() &&
                        self.selected.iter().all(|&idx| self.items.get(idx).map(|it| it.is_locked).unwrap_or(false));
                    if has_selection && all_selected_locked {
                        self.confirm_action = Some(ConfirmAction::Unlock);
                    }
                }

                // Delete: Remove selected items
                if i.key_pressed(egui::Key::Delete) {
                    if !self.selected.is_empty() {
                        self.confirm_action = Some(ConfirmAction::Remove);
                    }
                }

                // Escape: Clear selection
                if i.key_pressed(egui::Key::Escape) {
                    self.selected.clear();
                    self.last_selected = None;
                }
            });
        }

        // Process background/batched folder operations with parallel encryption
        if let Some(mut op) = self.current_op.take() {
            // Use dynamic thread count based on CPU cores and detected file sizes
            // Cap at 4 to prevent memory exhaustion (each operation uses ~48MB minimum)
            // Conservative limit ensures stable batch operations with large file counts
            let max_parallel = self.perf_config.thread_count.min(4);

            // Drain new paths from scanner
            if let Some(ref rx) = op.rx {
                loop {
                    match rx.try_recv() {
                        Ok(p) => op.queue.push_back(p),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            op.scanning_done = true;
                            op.rx = None;
                            break;
                        }
                    }
                }
            }

            // Collect results from completed background threads
            self.op_result_rxs.retain_mut(|rx| {
                match rx.try_recv() {
                    Ok((path, success, error_msg)) => {
                        if !success {
                            op.failures += 1;
                            if let Some(err) = error_msg {
                                op.error_details.push((path, err));
                            }
                        }
                        false  // Remove this receiver - operation completed
                    }
                    Err(TryRecvError::Disconnected) => {
                        false  // Remove - thread crashed or finished
                    }
                    Err(TryRecvError::Empty) => {
                        true   // Keep - still processing
                    }
                }
            });

            // Spawn new background threads up to max_parallel limit
            while self.op_result_rxs.len() < max_parallel && !op.queue.is_empty() {
                let Some(p) = op.queue.pop_front() else { break };

                let key = match &self.encryption_key {
                    Some(k) => *k,
                    None => {
                        op.failures += 1;
                        self.current_op = Some(op);
                        ctx.request_repaint();
                        return;
                    }
                };

                let (result_tx, result_rx) = mpsc::channel();
                let op_kind = op.kind;
                let p_clone = p.clone();
                let perf_config = self.perf_config.clone();
                std::thread::spawn(move || {
                    let res = match op_kind {
                        BatchOpKind::LockFolder => {
                            let out = MyVaultApp::encrypted_path_for(&p);

                            // Determine encryption strategy based on file size
                            let file_size = std::fs::metadata(&p)
                                .map(|m| m.len())
                                .unwrap_or(0);

                            // Use streaming encryption for all files to prevent memory exhaustion
                            // when processing many files in parallel. Streaming is memory-safe and
                            // provides good performance with the optimized 16MB chunks.
                            let encrypt_result = crate::crypto::encrypt_file_streaming(&key, &p, &out);

                            match encrypt_result {
                                Ok(_) => {
                                    let _ = crate::platform::hide(&out);
                                    let _ = std::fs::remove_file(&p);
                                    (true, None)
                                }
                                Err(e) => (false, Some(format!("Encryption failed: {}", e))),
                            }
                        }
                        BatchOpKind::UnlockFolder => {
                            let _ = crate::platform::unhide(&p);
                            if let Some(out) = MyVaultApp::original_path_for(&p) {
                                // Check if original file already exists
                                if out.exists() {
                                    (false, Some(format!("Original file exists: {}", out.display())))
                                } else {
                                    // Use streaming decryption for all files to prevent memory exhaustion
                                    // when processing many files in parallel. Streaming is memory-safe and
                                    // provides good performance with optimized chunk sizes.
                                    let decrypt_result = crate::crypto::decrypt_file_streaming(&key, &p, &out);

                                    match decrypt_result {
                                        Ok(_) => {
                                            // Force file handle cleanup
                                            drop(decrypt_result);
                                            let _ = std::fs::remove_file(&p);
                                            (true, None)
                                        }
                                        Err(e) => (false, Some(format!("Decryption failed: {}", e))),
                                    }
                                }
                            } else {
                                (false, Some(format!("Invalid encrypted filename")))
                            }
                        }
                    };
                    let _ = result_tx.send((p_clone, res.0, res.1));
                });

                // Yield to allow OS to clean up file handles between operations
                // Prevents file descriptor exhaustion on large batch operations (99+ files)
                std::thread::sleep(std::time::Duration::from_millis(5));

                op.processed += 1;
                self.op_result_rxs.push(result_rx);
            }

            // Complete only when scanning is done, queue is empty, AND all background threads finished
            if op.scanning_done && op.queue.is_empty() && self.op_result_rxs.is_empty() {
                // Update all affected items (not just the first one)
                for &idx in &op.affected_items {
                    if let Some(item) = self.items.get_mut(idx) {
                        match op.kind {
                            BatchOpKind::LockFolder => item.is_locked = true,
                            BatchOpKind::UnlockFolder => item.is_locked = false,
                        }
                    }
                }
                self.save_config();

                // Store error details for display
                if !op.error_details.is_empty() {
                    self.last_error_report = op.error_details.clone();
                    self.show_error_report = true;
                }

                // Calculate execution time
                let elapsed = op.start_time.elapsed();
                let elapsed_secs = elapsed.as_secs_f64();
                let time_str = if elapsed_secs < 1.0 {
                    format!("{:.2}ms", elapsed.as_millis())
                } else if elapsed_secs < 60.0 {
                    format!("{:.2}s", elapsed_secs)
                } else {
                    let mins = elapsed_secs / 60.0;
                    let secs = elapsed_secs % 60.0;
                    format!("{}m {:.1}s", mins as u32, secs)
                };

                let msg = match op.kind {
                    BatchOpKind::LockFolder => if op.failures == 0 {
                        format!("Locked {} items in {}", op.affected_items.len(), time_str)
                    } else {
                        format!("Locked {} items with {} errors in {} - click 'View Error Report' to see details", op.affected_items.len(), op.failures, time_str)
                    },
                    BatchOpKind::UnlockFolder => if op.failures == 0 {
                        format!("Unlocked {} items in {}", op.affected_items.len(), time_str)
                    } else {
                        format!("Unlocked {} items with {} errors in {} - click 'View Error Report' to see details", op.affected_items.len(), op.failures, time_str)
                    },
                };
                self.status_message = msg;
                // completed
            } else {
                self.current_op = Some(op);
                ctx.request_repaint();
            }
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("My Vault App");
                ui.separator();
                let mp_label = if self.master_password_hash.is_some() { "Master Password" } else { "Create Master Password" };
                if ui.button(mp_label).clicked() {
                    self.show_password_dialog = true;
                }
                // Add Change Password button (only show if password is already set and authenticated)
                if self.master_password_hash.is_some() && self.authenticated {
                    if ui.button("Change Password").clicked() {
                        self.show_change_password_dialog = true;
                    }
                }
                ui.separator();
                // Phase 1: Dark mode toggle
                let theme_label = if self.dark_mode { "☀ Light Mode" } else { "🌙 Dark Mode" };
                if ui.button(theme_label).clicked() {
                    self.dark_mode = !self.dark_mode;
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Files & folders list");
            ui.separator();

            ui.horizontal(|ui| {
                let busy = self.current_op.is_some();
                if ui.add_enabled(!busy, egui::Button::new("Add File"))
                    .on_hover_text("Add a single file to encrypt/decrypt")
                    .clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.add_path(path, ItemType::File);
                    }
                }
                if ui.add_enabled(!busy, egui::Button::new("Add Folder"))
                    .on_hover_text("Add a folder - all files will be processed")
                    .clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.add_path(path, ItemType::Folder);
                    }
                }

                if ui.add_enabled(!busy, egui::Button::new("Scan for Locked Files"))
                    .on_hover_text("Scan a folder for previously encrypted files")
                    .clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.scan_locked_files(&path);
                    }
                }

                // Check if all selected items are locked
                let all_selected_locked = !self.selected.is_empty() &&
                    self.selected.iter().all(|&i| self.items.get(i).map(|it| it.is_locked).unwrap_or(false));
                let some_selected_unlocked = !self.selected.is_empty() &&
                    self.selected.iter().any(|&i| self.items.get(i).map(|it| !it.is_locked).unwrap_or(false));
                let has_selection = !self.selected.is_empty();

                let can_lock = !busy && has_selection && self.authenticated && some_selected_unlocked;
                if ui.add_enabled(can_lock, egui::Button::new("Lock"))
                    .on_hover_text("Encrypt selected files (Ctrl+L)")
                    .clicked() {
                    self.confirm_action = Some(ConfirmAction::Lock);
                }

                let can_unlock = !busy && has_selection && self.authenticated && all_selected_locked;
                if ui.add_enabled(can_unlock, egui::Button::new("Unlock"))
                    .on_hover_text("Decrypt selected files (Ctrl+U)")
                    .clicked() {
                    self.confirm_action = Some(ConfirmAction::Unlock);
                }

                if ui.add_enabled(!busy && has_selection, egui::Button::new("Remove"))
                    .on_hover_text("Remove from list (doesn't delete files) (Delete)")
                    .clicked() {
                    self.confirm_action = Some(ConfirmAction::Remove);
                }

                // Show selection count
                if has_selection {
                    ui.label(format!("Selected: {}", self.selected.len()));
                }
            });

            // Phase 2: Search and sort controls
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.add(egui::TextEdit::singleline(&mut self.search_filter)
                    .hint_text("Filter by filename..."));

                if ui.small_button("✖").on_hover_text("Clear search").clicked() {
                    self.search_filter.clear();
                }

                ui.separator();

                ui.label("Sort by:");
                if ui.selectable_label(self.sort_by == SortField::Name, "Name").on_hover_text("Sort by filename").clicked() {
                    if self.sort_by == SortField::Name {
                        self.sort_ascending = !self.sort_ascending;
                    } else {
                        self.sort_by = SortField::Name;
                        self.sort_ascending = true;
                    }
                }
                if ui.selectable_label(self.sort_by == SortField::Status, "Status").on_hover_text("Sort by lock status").clicked() {
                    if self.sort_by == SortField::Status {
                        self.sort_ascending = !self.sort_ascending;
                    } else {
                        self.sort_by = SortField::Status;
                        self.sort_ascending = true;
                    }
                }
                if ui.selectable_label(self.sort_by == SortField::Size, "Size").on_hover_text("Sort by file size").clicked() {
                    if self.sort_by == SortField::Size {
                        self.sort_ascending = !self.sort_ascending;
                    } else {
                        self.sort_by = SortField::Size;
                        self.sort_ascending = true;
                    }
                }

                let arrow = if self.sort_ascending { "⬆" } else { "⬇" };
                ui.label(arrow);
            });

            ui.separator();

            // Dim the files list if not authenticated
            let enabled = self.authenticated;
            ui.set_enabled(enabled);

            // Show placeholder message if not authenticated
            if !enabled {
                ui.heading("🔒 Please enter password to view files");
            }

            // Phase 2: Prepare filtered and sorted items
            let mut display_items: Vec<(usize, &VaultItem)> = self.items.iter().enumerate()
                .filter(|(_, item)| {
                    // Filter by search string
                    if self.search_filter.is_empty() {
                        true
                    } else {
                        let search_lower = self.search_filter.to_lowercase();
                        item.original_path.to_string_lossy().to_lowercase().contains(&search_lower)
                    }
                })
                .collect();

            // Sort items
            display_items.sort_by(|(_, a), (_, b)| {
                let ordering = match self.sort_by {
                    SortField::Name => {
                        a.original_path.file_name().unwrap_or_default()
                            .to_string_lossy()
                            .cmp(&b.original_path.file_name().unwrap_or_default().to_string_lossy())
                    }
                    SortField::Status => {
                        a.is_locked.cmp(&b.is_locked)
                    }
                    SortField::Size => {
                        let size_a = std::fs::metadata(&a.original_path).map(|m| m.len()).unwrap_or(0);
                        let size_b = std::fs::metadata(&b.original_path).map(|m| m.len()).unwrap_or(0);
                        size_a.cmp(&size_b)
                    }
                };
                if self.sort_ascending {
                    ordering
                } else {
                    ordering.reverse()
                }
            });

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Phase 2: Drag and drop support
                if let Some(dropped_files) = ui.ctx().input(|i| {
                    if !i.raw.dropped_files.is_empty() {
                        Some(i.raw.dropped_files.clone())
                    } else {
                        None
                    }
                }) {
                    for file in dropped_files {
                        if let Some(path) = file.path {
                            let item_type = if path.is_dir() { ItemType::Folder } else { ItemType::File };
                            self.add_path(path, item_type);
                        }
                    }
                }

                // Show message if filtering resulted in empty list
                if display_items.is_empty() && !self.items.is_empty() {
                    ui.label("No items match the search filter");
                } else if self.items.is_empty() {
                    ui.label("No files added yet. Use buttons above or drag & drop files here.");
                }

                for (idx, item) in display_items.iter() {
                    let is_selected = self.selected.contains(idx);
                    let file_size = format_file_size(&item.original_path);
                    let label = format!(
                        "{}  {}  {}  {} {}",
                        match item.item_type { ItemType::File => "[F]", ItemType::Folder => "[D]" },
                        item.original_path.display(),
                        file_size,
                        if item.is_locked { "Locked" } else { "Unlocked" },
                        if item.is_locked { "🔒" } else { "🔓" }
                    );

                    // Multi-select with Ctrl+click and Shift+click for range selection
                    if ui.selectable_label(is_selected, label).clicked() {
                        let modifiers = ui.ctx().input(|i| i.modifiers);
                        if modifiers.shift {
                            // Range select with Shift held
                            if let Some(last) = self.last_selected {
                                let start = last.min(*idx);
                                let end = last.max(*idx);
                                for j in start..=end {
                                    self.selected.insert(j);
                                }
                            } else {
                                self.selected.insert(*idx);
                            }
                            self.last_selected = Some(*idx);
                        } else if modifiers.ctrl {
                            // Toggle with Ctrl held
                            if is_selected {
                                self.selected.remove(idx);
                            } else {
                                self.selected.insert(*idx);
                            }
                            self.last_selected = Some(*idx);
                        } else {
                            // Single select without modifiers
                            self.selected.clear();
                            self.selected.insert(*idx);
                            self.last_selected = Some(*idx);
                        }
                    }
                }
            });

            ui.set_enabled(true);  // Re-enable for rest of UI
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let msg = self.status_message.as_str();
                let err = msg.contains("error") || msg.contains("Error") || msg.contains("Invalid") || msg.contains("failed") || msg.contains("Failed");
                let ok = msg.contains("Locked") || msg.contains("Unlocked") || msg.contains("Authenticated") || msg.contains("Loaded") || msg.contains("Added") || msg.contains("created") || msg.contains("Removed");
                let color = if err {
                    egui::Color32::RED
                } else if ok {
                    egui::Color32::GREEN
                } else {
                    ui.visuals().text_color()
                };
                ui.colored_label(color, msg);

                // Show error report button if there are errors
                if !self.last_error_report.is_empty() && ui.button("View Error Report").clicked() {
                    self.show_error_report = true;
                }
            });
        });

        // Modal-like password dialog
        if self.show_password_dialog {
            let has_hash = self.master_password_hash.is_some();
            let title = if has_hash { "Enter Master Password" } else { "Create Master Password" };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if has_hash {
                            ui.label("Enter your master password:");
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.temp_password)
                                    .password(true)
                                    .hint_text("Password"),
                            );
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {}
                        } else {
                            ui.label("Create a master password (store it safely):");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.temp_password)
                                    .password(true)
                                    .hint_text("Password"),
                            );

                            // Phase 1: Password strength meter
                            let (strength_level, strength_color, strength_label) = assess_password_strength(&self.temp_password);
                            if !self.temp_password.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label("Strength:");
                                    // Visual strength bar
                                    let bar_width = 150.0;
                                    let bar_height = 8.0;
                                    let filled_width = bar_width * ((strength_level + 1) as f32 / 3.0);

                                    let (rect, _response) = ui.allocate_exact_size(
                                        egui::vec2(bar_width, bar_height),
                                        egui::Sense::hover()
                                    );

                                    // Draw background
                                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_gray(50));

                                    // Draw filled portion
                                    let filled_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(filled_width, bar_height)
                                    );
                                    ui.painter().rect_filled(filled_rect, 2.0, strength_color);

                                    ui.colored_label(strength_color, strength_label);
                                });
                            }

                            ui.add(
                                egui::TextEdit::singleline(&mut self.temp_password_confirm)
                                    .password(true)
                                    .hint_text("Confirm password"),
                            );
                        }

                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.show_password_dialog = false;
                                self.temp_password.zeroize();
                                self.temp_password.clear();
                                self.temp_password_confirm.zeroize();
                                self.temp_password_confirm.clear();
                            }

                            if has_hash {
                                if ui.button("Enter").clicked() {
                                    match (&self.master_password_hash).as_deref() {
                                        Some(hash) => match crypto::verify_password(&self.temp_password, hash) {
                                            Ok(true) => {
                                                // derive session key
                                                if let Some(salt) = &self.salt {
                                                    match crypto::derive_key(&self.temp_password, salt) {
                                                        Ok(k) => { self.encryption_key = Some(k); }
                                                        Err(e) => { self.status_message = format!("Key derivation error: {}", e); }
                                                    }
                                                }
                                                self.authenticated = true;
                                                self.status_message = "Authenticated".to_string();
                                                self.show_password_dialog = false;
                                            }
                                            Ok(false) => {
                                                self.status_message = "Invalid password".to_string();
                                            }
                                            Err(e) => {
                                                self.status_message = format!("Password verification error: {}", e);
                                            }
                                        },
                                        None => {}
                                    }
                                    self.temp_password.zeroize();
                                    self.temp_password.clear();
                                }
                            } else {
                                if ui.button("Create").clicked() {
                                    if self.temp_password.is_empty() {
                                        self.status_message = "Password cannot be empty".to_string();
                                    } else if self.temp_password != self.temp_password_confirm {
                                        self.status_message = "Passwords do not match".to_string();
                                    } else {
                                        match crypto::hash_password(&self.temp_password) {
                                            Ok((hash, salt)) => {
                                                self.master_password_hash = Some(hash);
                                                self.salt = Some(salt);
                                                // derive session key
                                                if let Some(salt) = &self.salt {
                                                    match crypto::derive_key(&self.temp_password, salt) {
                                                        Ok(k) => { self.encryption_key = Some(k); }
                                                        Err(e) => { self.status_message = format!("Key derivation error: {}", e); }
                                                    }
                                                }
                                                self.authenticated = true;
                                                self.status_message = "Master password created".to_string();
                                                self.save_config();
                                                self.show_password_dialog = false;
                                            }
                                            Err(e) => {
                                                self.status_message = format!("Hashing error: {}", e);
                                            }
                                        }
                                    }
                                    self.temp_password.zeroize();
                                    self.temp_password.clear();
                                    self.temp_password_confirm.zeroize();
                                    self.temp_password_confirm.clear();
                                }
                            }
                        });
                    });
                });
        }

        // Change password dialog (only show when authenticated)
        if self.show_change_password_dialog && self.authenticated {
            egui::Window::new("Change Master Password")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Current password:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.current_password)
                            .password(true)
                            .hint_text("Current password"),
                    );

                    ui.separator();

                    ui.label("New password:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_password)
                            .password(true)
                            .hint_text("New password"),
                    );

                    // Phase 1: Password strength meter for new password
                    let (strength_level, strength_color, strength_label) = assess_password_strength(&self.new_password);
                    if !self.new_password.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Strength:");
                            // Visual strength bar
                            let bar_width = 150.0;
                            let bar_height = 8.0;
                            let filled_width = bar_width * ((strength_level + 1) as f32 / 3.0);

                            let (rect, _response) = ui.allocate_exact_size(
                                egui::vec2(bar_width, bar_height),
                                egui::Sense::hover()
                            );

                            // Draw background
                            ui.painter().rect_filled(rect, 2.0, egui::Color32::from_gray(50));

                            // Draw filled portion
                            let filled_rect = egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2(filled_width, bar_height)
                            );
                            ui.painter().rect_filled(filled_rect, 2.0, strength_color);

                            ui.colored_label(strength_color, strength_label);
                        });
                    }

                    ui.label("Confirm new password:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_password_confirm)
                            .password(true)
                            .hint_text("Confirm new password"),
                    );

                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_change_password_dialog = false;
                            self.current_password.zeroize();
                            self.current_password.clear();
                            self.new_password.zeroize();
                            self.new_password.clear();
                            self.new_password_confirm.zeroize();
                            self.new_password_confirm.clear();
                        }

                        if ui.button("Change Password").clicked() {
                            // Verify current password
                            if let Some(hash) = &self.master_password_hash {
                                match crypto::verify_password(&self.current_password, hash) {
                                    Ok(true) => {
                                        // Validate new password
                                        if self.new_password.is_empty() {
                                            self.status_message = "New password cannot be empty".to_string();
                                        } else if self.new_password != self.new_password_confirm {
                                            self.status_message = "New passwords do not match".to_string();
                                        } else {
                                            // Hash the new password
                                            match crypto::hash_password(&self.new_password) {
                                                Ok((new_hash, new_salt)) => {
                                                    // Update the stored hash and salt
                                                    self.master_password_hash = Some(new_hash);
                                                    self.salt = Some(new_salt.clone());

                                                    // Re-derive the encryption key with the new password and salt
                                                    match crypto::derive_key(&self.new_password, &new_salt) {
                                                        Ok(new_key) => {
                                                            self.encryption_key = Some(new_key);
                                                            self.save_config();
                                                            self.status_message = "Master password changed successfully".to_string();
                                                            self.show_change_password_dialog = false;
                                                        }
                                                        Err(e) => {
                                                            self.status_message = format!("Key derivation error: {}", e);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    self.status_message = format!("Hashing error: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Ok(false) => {
                                        self.status_message = "Current password is incorrect".to_string();
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Password verification error: {}", e);
                                    }
                                }
                            }

                            // Clear password fields
                            self.current_password.zeroize();
                            self.current_password.clear();
                            self.new_password.zeroize();
                            self.new_password.clear();
                            self.new_password_confirm.zeroize();
                            self.new_password_confirm.clear();
                        }
                    });
                });
        }

        // Confirmation dialog for lock/unlock/remove/overwrite
        if !self.show_password_dialog && !self.show_change_password_dialog {
            if let Some(action) = self.confirm_action.clone() {
                let title = match action {
                    ConfirmAction::Lock => "Confirm Lock",
                    ConfirmAction::Unlock => "Confirm Unlock",
                    ConfirmAction::Remove => "Confirm Remove",
                    ConfirmAction::OverwriteLock { .. } | ConfirmAction::OverwriteUnlock { .. } => "Confirm Overwrite",
                    ConfirmAction::ChangePassword => "Confirm Change Password",
                };
                egui::Window::new(title)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        let item_desc = if let Some(&i) = self.selected.iter().next() {
                            self.items.get(i)
                                .map(|it| format!("{}", it.original_path.display()))
                                .unwrap_or_else(|| "<none>".to_string())
                        } else {
                            "<none>".to_string()
                        };
                        match action {
                            ConfirmAction::Lock => ui.label("This will encrypt and hide the selected item."),
                            ConfirmAction::Unlock => ui.label("This will decrypt and restore the selected item."),
                            ConfirmAction::Remove => ui.label("This removes the item from the list only; it does not delete files."),
                            ConfirmAction::OverwriteLock { ref dst, .. } => ui.label(format!("Encrypted file exists: {}\nOverwrite?", dst.display())),
                            ConfirmAction::OverwriteUnlock { ref dst, .. } => ui.label(format!("Original file exists: {}\nOverwrite?", dst.display())),
                            ConfirmAction::ChangePassword => ui.label("This will update your master password."),
                        };
                        ui.label(format!("Item: {}",
                            item_desc
                        ));
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.confirm_action = None;
                            }
                            let confirm_label = match action {
                                ConfirmAction::Lock => "Lock",
                                ConfirmAction::Unlock => "Unlock",
                                ConfirmAction::Remove => "Remove",
                                ConfirmAction::OverwriteLock { .. } => "Overwrite",
                                ConfirmAction::OverwriteUnlock { .. } => "Overwrite",
                                ConfirmAction::ChangePassword => "Change",
                            };
                            if ui.button(confirm_label).clicked() {
                                match action {
                                    ConfirmAction::Lock => self.lock_selected(),
                                    ConfirmAction::Unlock => self.unlock_selected(),
                                    ConfirmAction::Remove => self.remove_selected(),
                                    ConfirmAction::OverwriteLock { src, dst } => {
                                        if let Err(e) = self.perform_overwrite_lock(&src, &dst) {
                                            self.status_message = format!("Overwrite failed: {}", e);
                                        }
                                    }
                                    ConfirmAction::OverwriteUnlock { src, dst } => {
                                        if let Err(e) = self.perform_overwrite_unlock(&src, &dst) {
                                            self.status_message = format!("Overwrite failed: {}", e);
                                        }
                                    }
                                    ConfirmAction::ChangePassword => {
                                        // This action is handled directly in the change password dialog
                                    }
                                }
                                self.confirm_action = None;
                            }
                        });
                    });
            }
        }

        // Error report window
        let mut close_error_report = false;
        if self.show_error_report && !self.last_error_report.is_empty() {
            let error_count = self.last_error_report.len();
            let errors = self.last_error_report.clone();

            egui::Window::new("Error Report")
                .collapsible(false)
                .resizable(true)
                .default_width(600.0)
                .default_height(400.0)
                .show(ctx, |ui| {
                    ui.label(format!("Failed files: {} total errors", error_count));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for (idx, (path, error)) in errors.iter().enumerate() {
                                ui.vertical(|ui| {
                                    ui.colored_label(
                                        egui::Color32::LIGHT_GRAY,
                                        format!("{}. {}", idx + 1, path.display()),
                                    );
                                    ui.colored_label(egui::Color32::RED, format!("   └─ {}", error));
                                });
                            }
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close_error_report = true;
                        }
                        if ui.button("Copy to Clipboard").clicked() {
                            let report = errors
                                .iter()
                                .enumerate()
                                .map(|(i, (p, e))| format!("{}. {}\n   Error: {}\n", i + 1, p.display(), e))
                                .collect::<String>();

                            // Phase 1: Clipboard support implementation
                            match platform::set_clipboard(&report) {
                                Ok(_) => {
                                    self.status_message = format!("Error report copied to clipboard ({} items)", errors.len());
                                }
                                Err(e) => {
                                    self.status_message = format!("Failed to copy to clipboard: {}", e);
                                }
                            }
                        }
                    });
                });
        }
        if close_error_report {
            self.show_error_report = false;
        }

        // Progress window for current folder operation
        if let Some(op) = self.current_op.as_ref() {
            let scanning_done = op.scanning_done;
            let processed = op.processed;
            let queue_len = op.queue.len();
            let failures = op.failures;
            let kind = op.kind;
            let start_time = op.start_time;

            // Phase 1: Calculate throughput and ETA
            let elapsed = start_time.elapsed().as_secs_f32();
            let throughput = if elapsed > 0.0 && processed > 0 {
                processed as f32 / elapsed
            } else {
                0.0
            };

            let (progress, text, eta_text) = if scanning_done {
                let total = processed + queue_len;
                let pct = if total == 0 { 0.0 } else { processed as f32 / total as f32 };
                let eta = if throughput > 0.0 && queue_len > 0 {
                    let remaining_secs = queue_len as f32 / throughput;
                    if remaining_secs < 60.0 {
                        format!("ETA: {:.0}s", remaining_secs)
                    } else if remaining_secs < 3600.0 {
                        format!("ETA: {:.1}m", remaining_secs / 60.0)
                    } else {
                        format!("ETA: {:.1}h", remaining_secs / 3600.0)
                    }
                } else {
                    String::new()
                };
                let throughput_str = if throughput > 0.0 {
                    format!("Speed: {:.1} files/s", throughput)
                } else {
                    String::new()
                };
                (pct, format!("Processed {} of {} ({} errors)", processed, total, failures), format!("{} {}", throughput_str, eta).trim().to_string())
            } else {
                (0.0, format!("Scanning... processed {} (+{} queued), {} errors", processed, queue_len, failures), String::new())
            };
            let title = match kind { BatchOpKind::LockFolder => "Locking Folder", BatchOpKind::UnlockFolder => "Unlocking Folder" };
            let mut cancel_clicked = false;
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&text);
                    if !eta_text.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(100, 149, 237), &eta_text);
                    }
                    if scanning_done {
                        ui.add(egui::widgets::ProgressBar::new(progress).show_percentage());
                    } else {
                        ui.add(egui::Spinner::new());
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            if cancel_clicked {
                self.status_message = match kind {
                    BatchOpKind::LockFolder => format!("Canceled lock; processed {} files", processed),
                    BatchOpKind::UnlockFolder => format!("Canceled unlock; processed {} files", processed),
                };
                self.current_op = None;
            }
        }
    }
}

/// Phase 2: Get file size with human-readable format
fn format_file_size(path: &Path) -> String {
    if let Ok(metadata) = std::fs::metadata(path) {
        let size = metadata.len();
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    } else {
        "N/A".to_string()
    }
}

/// Phase 1: Password strength assessment
/// Returns (strength_level, color, label)
/// - Level 0 (Weak): < 8 chars or simple patterns
/// - Level 1 (Medium): 8-11 chars with some complexity
/// - Level 2 (Strong): 12+ chars with high complexity
fn assess_password_strength(password: &str) -> (u8, egui::Color32, &'static str) {
    if password.is_empty() {
        return (0, egui::Color32::GRAY, "");
    }

    let len = password.len();
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_digit = password.chars().any(|c| c.is_numeric());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    let complexity = [has_lower, has_upper, has_digit, has_special]
        .iter()
        .filter(|&&x| x)
        .count();

    // Check for common patterns
    let is_sequential = password.chars().collect::<Vec<_>>().windows(3).any(|w| {
        if w.len() == 3 {
            let c1 = w[0] as i32;
            let c2 = w[1] as i32;
            let c3 = w[2] as i32;
            (c2 - c1 == 1 && c3 - c2 == 1) || (c1 - c2 == 1 && c2 - c3 == 1)
        } else {
            false
        }
    });

    let is_repetitive = password.chars().collect::<Vec<_>>().windows(3).any(|w| {
        w.len() == 3 && w[0] == w[1] && w[1] == w[2]
    });

    // Scoring logic
    if len < 8 || is_sequential || is_repetitive {
        (0, egui::Color32::from_rgb(220, 53, 69), "Weak")
    } else if len >= 12 && complexity >= 3 {
        (2, egui::Color32::from_rgb(40, 167, 69), "Strong")
    } else if len >= 8 && complexity >= 2 {
        (1, egui::Color32::from_rgb(255, 193, 7), "Medium")
    } else {
        (0, egui::Color32::from_rgb(220, 53, 69), "Weak")
    }
}

#[derive(Debug)]
struct BatchOp {
    kind: BatchOpKind,
    queue: VecDeque<PathBuf>,
    rx: Option<Receiver<PathBuf>>,
    scanning_done: bool,
    processed: usize,
    failures: usize,
    item_index: usize,
    affected_items: Vec<usize>,  // All item indices involved in this batch operation
    error_details: Vec<(PathBuf, String)>,  // Detailed error tracking: (file_path, error_reason)
    start_time: Instant,  // Track operation start time
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchOpKind {
    LockFolder,
    UnlockFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfirmAction {
    Lock,
    Unlock,
    Remove,
    OverwriteLock { src: PathBuf, dst: PathBuf },
    OverwriteUnlock { src: PathBuf, dst: PathBuf },
    ChangePassword,
}
