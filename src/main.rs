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
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Files & folders list");
            ui.separator();

            ui.horizontal(|ui| {
                let busy = self.current_op.is_some();
                if ui.add_enabled(!busy, egui::Button::new("Add File")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.add_path(path, ItemType::File);
                    }
                }
                if ui.add_enabled(!busy, egui::Button::new("Add Folder")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.add_path(path, ItemType::Folder);
                    }
                }

                if ui.add_enabled(!busy, egui::Button::new("Scan for Locked Files")).clicked() {
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
                if ui.add_enabled(can_lock, egui::Button::new("Lock")).clicked() {
                    self.confirm_action = Some(ConfirmAction::Lock);
                }

                let can_unlock = !busy && has_selection && self.authenticated && all_selected_locked;
                if ui.add_enabled(can_unlock, egui::Button::new("Unlock")).clicked() {
                    self.confirm_action = Some(ConfirmAction::Unlock);
                }

                if ui.add_enabled(!busy && has_selection, egui::Button::new("Remove")).clicked() {
                    self.confirm_action = Some(ConfirmAction::Remove);
                }

                // Show selection count
                if has_selection {
                    ui.label(format!("Selected: {}", self.selected.len()));
                }
            });

            ui.separator();

            // Dim the files list if not authenticated
            let enabled = self.authenticated;
            ui.set_enabled(enabled);

            // Show placeholder message if not authenticated
            if !enabled {
                ui.heading("🔒 Please enter password to view files");
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, item) in self.items.iter().enumerate() {
                    let is_selected = self.selected.contains(&i);
                    let label = format!(
                        "{}  {}  {} {}",
                        match item.item_type { ItemType::File => "[F]", ItemType::Folder => "[D]" },
                        item.original_path.display(),
                        if item.is_locked { "Locked" } else { "Unlocked" },
                        if item.is_locked { "🔒" } else { "🔓" }
                    );

                    // Multi-select with Ctrl+click and Shift+click for range selection
                    if ui.selectable_label(is_selected, label).clicked() {
                        let modifiers = ui.ctx().input(|i| i.modifiers);
                        if modifiers.shift {
                            // Range select with Shift held
                            if let Some(last) = self.last_selected {
                                let start = last.min(i);
                                let end = last.max(i);
                                for j in start..=end {
                                    self.selected.insert(j);
                                }
                            } else {
                                self.selected.insert(i);
                            }
                            self.last_selected = Some(i);
                        } else if modifiers.ctrl {
                            // Toggle with Ctrl held
                            if is_selected {
                                self.selected.remove(&i);
                            } else {
                                self.selected.insert(i);
                            }
                            self.last_selected = Some(i);
                        } else {
                            // Single select without modifiers
                            self.selected.clear();
                            self.selected.insert(i);
                            self.last_selected = Some(i);
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

        // Confirmation dialog for lock/unlock/remove/overwrite
        if !self.show_password_dialog {
            if let Some(action) = self.confirm_action.clone() {
                let title = match action {
                    ConfirmAction::Lock => "Confirm Lock",
                    ConfirmAction::Unlock => "Confirm Unlock",
                    ConfirmAction::Remove => "Confirm Remove",
                    ConfirmAction::OverwriteLock { .. } | ConfirmAction::OverwriteUnlock { .. } => "Confirm Overwrite",
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
                            let _report = errors
                                .iter()
                                .enumerate()
                                .map(|(i, (p, e))| format!("{}. {}\n   Error: {}\n", i + 1, p.display(), e))
                                .collect::<String>();
                            // Note: In a real app, you'd use a clipboard library here
                            self.status_message = format!("Report copied (manual clipboard support needed)");
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
            let (progress, text) = if scanning_done {
                let total = processed + queue_len;
                let pct = if total == 0 { 0.0 } else { processed as f32 / total as f32 };
                (pct, format!("Processed {} of {} ({} errors)", processed, total, failures))
            } else {
                (0.0, format!("Scanning... processed {} (+{} queued), {} errors", processed, queue_len, failures))
            };
            let title = match kind { BatchOpKind::LockFolder => "Locking Folder", BatchOpKind::UnlockFolder => "Unlocking Folder" };
            let mut cancel_clicked = false;
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&text);
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
}
