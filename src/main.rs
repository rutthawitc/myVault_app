#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod crypto;
mod model;
mod platform;
mod performance;

use performance::PerformanceConfig;

use eframe::egui;
use zeroize::Zeroize;
use model::{ItemType, VaultItem};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::{VecDeque, HashSet, BTreeMap};
use std::time::{Instant, SystemTime};

/// Name of the primary modifier key, for hover hints. egui maps `Modifiers::command`
/// to Cmd on macOS and Ctrl elsewhere, so the shortcuts themselves are already
/// portable - only the label has to change.
#[cfg(target_os = "macos")]
const CMD_LABEL: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const CMD_LABEL: &str = "Ctrl";

fn main() -> eframe::Result<()> {
    let mut options = eframe::NativeOptions::default();

    // Set window icon with a simple vault icon (lock emoji-based)
    options.viewport.icon = Some(std::sync::Arc::new(create_vault_icon()));
    // The toolbar and the file list both need room. A minimum stops the window
    // being dragged narrow enough to clip the toolbar's trailing buttons.
    options.viewport = options
        .viewport
        .with_inner_size([1000.0, 700.0])
        .with_min_inner_size([720.0, 480.0]);

    eframe::run_native(
        "My Vault App",
        options,
        Box::new(|cc| {
            install_unicode_font(&cc.egui_ctx);
            configure_style(&cc.egui_ctx);
            Ok(Box::new(MyVaultApp::new()))
        }),
    )
}

/// Loosen up egui's defaults, which are tuned for dense debug UIs.
///
/// Spacing and text sizes live in `Style` and survive the per-theme
/// `set_visuals` call, so this only has to run once at startup.
fn configure_style(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.menu_margin = egui::Margin::same(6);

        // The stock 12.5pt body text is small for a file list read at arm's length.
        for (text_style, size) in [
            (egui::TextStyle::Body, 14.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Heading, 20.0),
        ] {
            if let Some(font) = style.text_styles.get_mut(&text_style) {
                font.size = size;
            }
        }
    });
}

/// Apply the light/dark palette, with the app's own corner rounding on top.
///
/// `set_visuals` replaces the whole palette, so the rounding has to be reapplied
/// with it - but only when the theme actually changes, not every frame.
fn apply_theme(ctx: &egui::Context, dark_mode: bool) {
    let mut visuals = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(5);
    }

    ctx.set_visuals(visuals);
}

/// Add a system font that covers Thai to the font stack.
///
/// egui ships only Latin/Greek/Cyrillic faces, so a file named in Thai renders as a
/// row of tofu boxes. There is no bundled font in this repo, so we look for one the
/// OS already provides and append it as a *fallback* - the default face still wins
/// for Latin text, and Thai codepoints fall through to this one.
///
/// If nothing is found the app keeps working exactly as before; this is a
/// best-effort enhancement, never a startup failure.
fn install_unicode_font(ctx: &egui::Context) {
    // Ordered best-first per platform. `.ttc` collections are fine: `FontData`
    // carries a face `index`, and 0 is the regular face in all of these.
    const CANDIDATES: &[&str] = &[
        // macOS
        "/System/Library/Fonts/Supplemental/Thonburi.ttc",
        "/System/Library/Fonts/ThonburiUI.ttc",
        "/System/Library/Fonts/Supplemental/Ayuthaya.ttf",
        // Windows
        "C:\\Windows\\Fonts\\leelawui.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
        // Linux
        "/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansThai-Regular.ttf",
        "/usr/share/fonts/TTF/NotoSansThai-Regular.ttf",
        "/usr/share/fonts/truetype/tlwg/Loma.ttf",
    ];

    let Some(bytes) = CANDIDATES
        .iter()
        .find_map(|path| std::fs::read(path).ok())
    else {
        return;
    };

    const KEY: &str = "system_unicode";
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(KEY.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));

    // Append, never prepend: the default face keeps rendering Latin text, and only
    // glyphs it does not have (Thai) fall through to the system font.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(KEY.to_owned());
    }

    ctx.set_fonts(fonts);
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
    /// The Data Encryption Key, unwrapped in memory while the vault is unlocked.
    /// It is independent of the master password, so changing the password does
    /// not invalidate previously encrypted files.
    encryption_key: Option<crypto::SecretKey>,
    wrapped_dek: Option<String>,
    confirm_action: Option<ConfirmAction>,
    current_op: Option<BatchOp>,
    /// One entry per in-flight worker: the file it is working on, and the channel
    /// it will report back on. The path is kept so the progress window can name
    /// what is being processed right now.
    op_result_rxs: Vec<(PathBuf, Receiver<(PathBuf, WorkerOutcome)>)>,
    show_error_report: bool,
    last_error_report: Vec<(PathBuf, String)>,
    perf_config: PerformanceConfig,  // Dynamic performance configuration based on CPU cores
    show_change_password_dialog: bool,
    current_password: String,
    new_password: String,
    new_password_confirm: String,
    dark_mode: bool,  // Phase 1: Dark mode toggle
    /// Which theme is currently installed, so it is only rebuilt when it changes.
    applied_dark_mode: Option<bool>,
    /// Reveal the characters in the password fields of the open dialog.
    /// Always reset to false when a dialog closes.
    reveal_passwords: bool,
    // Phase 2: UX Improvements
    search_filter: String,
    recent_files: Vec<PathBuf>,
    sort_by: SortField,
    sort_ascending: bool,
    // Phase 3: Security & Persistence
    last_activity: Instant,
    session_timeout_minutes: u64,
    auto_lock_enabled: bool,
    password_last_changed: Option<SystemTime>,
    password_change_reminder_days: u64,
    show_password_generator: bool,
    generated_password: String,
    show_settings_dialog: bool,
    // Password generator settings
    gen_length: usize,
    gen_use_lowercase: bool,
    gen_use_uppercase: bool,
    gen_use_digits: bool,
    gen_use_symbols: bool,
    // Password reminder tracking
    show_password_reminder: bool,
    reminder_dismissed_until: Option<SystemTime>,
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
            wrapped_dek: None,
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
            applied_dark_mode: None,
            reveal_passwords: false,
            // Phase 2: UX Improvements
            search_filter: String::new(),
            recent_files: Vec::new(),
            sort_by: SortField::Name,
            sort_ascending: true,
            // Phase 3: Security & Persistence
            last_activity: Instant::now(),
            session_timeout_minutes: 15,  // Default: 15 minutes
            auto_lock_enabled: true,  // Default: enabled for security
            password_last_changed: None,  // Will be loaded from config
            password_change_reminder_days: 90,  // Default: 90 days
            show_password_generator: false,
            generated_password: String::new(),
            show_settings_dialog: false,
            // Password generator settings
            gen_length: 16,  // Default: 16 characters
            gen_use_lowercase: true,
            gen_use_uppercase: true,
            gen_use_digits: true,
            gen_use_symbols: true,
            // Password reminder tracking
            show_password_reminder: false,
            reminder_dismissed_until: None,
        };
        app.load_from_config();
        // Nothing in the app works until the vault is unlocked, so ask straight
        // away instead of leaving the user to find the button in the toolbar.
        // With no vault yet this is the "create a master password" prompt.
        app.show_password_dialog = true;
        app
    }

    /// Is a dialog currently on screen?
    ///
    /// Keyboard shortcuts are suppressed while one is open, so that Escape closes
    /// the dialog instead of also clearing the selection behind it.
    fn any_dialog_open(&self) -> bool {
        self.show_password_dialog
            || self.show_change_password_dialog
            || self.show_password_generator
            || self.show_settings_dialog
            || self.show_error_report
            || self.confirm_action.is_some()
            || self.current_op.is_some()
    }

    /// Lock the vault: drop the key and ask for the password again.
    ///
    /// Used by the toolbar, by Settings, and by the auto-lock timer, so all three
    /// leave the app in exactly the same state.
    fn lock_session(&mut self, message: &str) {
        self.authenticated = false;
        self.encryption_key = None;
        // A selection made before locking should not survive into the next
        // session and become the target of a Lock the user did not line up.
        self.selected.clear();
        self.last_selected = None;
        self.status_message = message.to_string();
        self.last_activity = Instant::now();
        self.show_password_dialog = true;
    }

    /// Dismiss the unlock/create dialog, wiping whatever was typed into it.
    /// Shared by the Cancel button and by Escape / clicking outside the modal, so
    /// no path can leave a password sitting in memory.
    fn close_password_dialog(&mut self) {
        self.show_password_dialog = false;
        self.reveal_passwords = false;
        self.temp_password.zeroize();
        self.temp_password.clear();
        self.temp_password_confirm.zeroize();
        self.temp_password_confirm.clear();
    }

    /// Same, for the change-password dialog.
    fn close_change_password_dialog(&mut self) {
        self.show_change_password_dialog = false;
        self.reveal_passwords = false;
        self.current_password.zeroize();
        self.current_password.clear();
        self.new_password.zeroize();
        self.new_password.clear();
        self.new_password_confirm.zeroize();
        self.new_password_confirm.clear();
    }

    /// Seconds left before the session auto-locks, if auto-lock is on.
    fn auto_lock_remaining(&self) -> Option<u64> {
        if !self.auto_lock_enabled || !self.authenticated {
            return None;
        }
        let timeout = self.session_timeout_minutes.saturating_mul(60);
        Some(timeout.saturating_sub(self.last_activity.elapsed().as_secs()))
    }

    /// Set up key material for a brand-new vault.
    ///
    /// Generates a random Data Encryption Key and stores it wrapped with the
    /// key derived from the master password.
    fn create_vault_key(&mut self, password: &str, salt: &str) -> Result<(), String> {
        let mut kek = crypto::derive_key(password, salt)?;
        let dek = crypto::generate_dek();
        let wrapped = crypto::wrap_dek(&kek, &dek);
        kek.zeroize();

        self.wrapped_dek = Some(wrapped?);
        self.encryption_key = Some(crypto::SecretKey::new(dek));
        Ok(())
    }

    /// Unlock the vault: derive the key-encryption key from the password and
    /// recover the Data Encryption Key used to encrypt files.
    ///
    /// Vaults created before envelope encryption have no wrapped DEK stored. For
    /// those, the file key *was* the password-derived key, so we adopt that key as
    /// the DEK and wrap it in place. Existing files stay decryptable, and from then
    /// on a password change only re-wraps the DEK instead of orphaning every file.
    fn unlock_vault_key(&mut self, password: &str) -> Result<(), String> {
        let salt = self
            .salt
            .clone()
            .ok_or("Vault salt is missing from the configuration")?;
        let mut kek = crypto::derive_key(password, &salt)?;

        let existing = self.wrapped_dek.clone();
        let outcome = match existing {
            Some(w) => crypto::unwrap_dek(&kek, &w).map(|dek| (dek, None)),
            // Legacy vault: adopt the derived key as the DEK and wrap it.
            None => crypto::wrap_dek(&kek, &kek).map(|w| (kek, Some(w))),
        };
        kek.zeroize();

        let (dek, newly_wrapped) = outcome?;
        self.encryption_key = Some(crypto::SecretKey::new(dek));

        if let Some(w) = newly_wrapped {
            self.wrapped_dek = Some(w);
            self.save_config();
        }
        Ok(())
    }

    /// Re-wrap the existing Data Encryption Key with a new password.
    ///
    /// This is what makes a password change safe: the DEK never changes, so every
    /// file encrypted under the old password remains decryptable.
    fn rewrap_vault_key(&mut self, new_password: &str, new_salt: &str) -> Result<(), String> {
        let dek = self
            .encryption_key
            .as_ref()
            .ok_or("Vault is locked - unlock it before changing the master password")?
            .clone();
        let mut kek = crypto::derive_key(new_password, new_salt)?;
        let wrapped = crypto::wrap_dek(&kek, dek.as_bytes());
        kek.zeroize();

        self.wrapped_dek = Some(wrapped?);
        Ok(())
    }

    fn load_from_config(&mut self) {
        match config::load_config() {
            Ok(cfg) => {
                self.master_password_hash = cfg.master_password_hash;
                self.salt = cfg.salt;
                self.wrapped_dek = cfg.wrapped_dek;
                self.items = cfg.vault_items.iter().map(|c| c.into()).collect();

                // Configs written before sizes were cached have none. Measure them
                // once here rather than in the paint loop; from now on they persist.
                for item in &mut self.items {
                    if item.size.is_none() {
                        item.size = measure_size(&item.original_path);
                    }
                }

                // Phase 2 & 3: Restore UI preferences
                self.dark_mode = cfg.dark_mode;
                self.sort_by = match cfg.sort_by.as_str() {
                    "Status" => SortField::Status,
                    "Size" => SortField::Size,
                    _ => SortField::Name,
                };
                self.sort_ascending = cfg.sort_ascending;
                self.recent_files = cfg.recent_files.iter().map(PathBuf::from).collect();

                // Phase 3: Restore security settings
                self.session_timeout_minutes = cfg.session_timeout_minutes;
                self.auto_lock_enabled = cfg.auto_lock_enabled;
                self.password_change_reminder_days = cfg.password_change_reminder_days;
                self.password_last_changed = cfg.password_last_changed.map(|timestamp| {
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp)
                });
                self.reminder_dismissed_until = cfg.reminder_dismissed_until.map(|timestamp| {
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp)
                });

                self.status_message = "Loaded configuration".to_string();
            }
            Err(e) => {
                self.status_message = format!("Failed to load config: {}", e);
            }
        }
    }

    fn save_config(&mut self) {
        // Convert SystemTime to Unix timestamp for storage
        let password_timestamp = self.password_last_changed.and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });

        let reminder_timestamp = self.reminder_dismissed_until.and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });

        // Convert SortField to String
        let sort_by_str = match self.sort_by {
            SortField::Name => "Name",
            SortField::Status => "Status",
            SortField::Size => "Size",
        };

        // Convert Vec<PathBuf> to Vec<String>
        let recent_files_strings: Vec<String> = self.recent_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let cfg = config::Config {
            master_password_hash: self.master_password_hash.clone(),
            salt: self.salt.clone(),
            wrapped_dek: self.wrapped_dek.clone(),
            vault_items: self.items.iter().map(config::ConfigItem::from).collect(),
            dark_mode: self.dark_mode,
            sort_by: sort_by_str.to_string(),
            sort_ascending: self.sort_ascending,
            recent_files: recent_files_strings,
            session_timeout_minutes: self.session_timeout_minutes,
            auto_lock_enabled: self.auto_lock_enabled,
            password_change_reminder_days: self.password_change_reminder_days,
            password_last_changed: password_timestamp,
            reminder_dismissed_until: reminder_timestamp,
        };

        if let Err(e) = config::save_config(&cfg) {
            self.status_message = format!("Failed to save config: {}", e);
        }
    }

    // Phase 3: Add file to recent files list (keep last 20)
    fn add_to_recent_files(&mut self, path: PathBuf) {
        // Remove if already exists (to move it to front)
        self.recent_files.retain(|p| p != &path);
        // Add to front
        self.recent_files.insert(0, path);
        // Keep only last 20
        if self.recent_files.len() > 20 {
            self.recent_files.truncate(20);
        }
    }

    // Phase 3: Check if password change reminder should be shown
    fn check_password_reminder(&mut self) {
        // Don't show if reminder was dismissed recently
        if let Some(dismissed_until) = self.reminder_dismissed_until {
            if SystemTime::now() < dismissed_until {
                return; // Still in the "remind me later" period
            }
        }

        // Check if password is old enough to warrant a reminder
        if let Some(last_changed) = self.password_last_changed {
            if let Ok(age) = SystemTime::now().duration_since(last_changed) {
                let age_days = age.as_secs() / 86400; // Convert to days
                if age_days >= self.password_change_reminder_days {
                    self.show_password_reminder = true;
                }
            }
        }
    }

    // Phase 3: Generate secure random password
    fn generate_password(&self) -> String {
        use rand::Rng;

        let mut charset = Vec::new();
        if self.gen_use_lowercase {
            charset.extend_from_slice(b"abcdefghijklmnopqrstuvwxyz");
        }
        if self.gen_use_uppercase {
            charset.extend_from_slice(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        }
        if self.gen_use_digits {
            charset.extend_from_slice(b"0123456789");
        }
        if self.gen_use_symbols {
            charset.extend_from_slice(b"!@#$%^&*()-_=+[]{}|;:,.<>?");
        }

        // If no character sets selected, use all
        if charset.is_empty() {
            charset.extend_from_slice(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:,.<>?");
        }

        let mut rng = rand::thread_rng();
        (0..self.gen_length)
            .map(|_| charset[rng.gen_range(0..charset.len())] as char)
            .collect()
    }

    fn add_path(&mut self, path: PathBuf, item_type: ItemType) {
        let item = VaultItem {
            size: measure_size(&path),
            original_path: path,
            encrypted_path: None,
            is_locked: false,
            item_type,
            is_folder_hidden: false,
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

        // Count before clearing the selection - reading the length afterwards
        // always reported zero.
        let mut removed = 0;
        for i in indices {
            if i < self.items.len() {
                self.items.remove(i);
                removed += 1;
            }
        }

        self.selected.clear();
        self.last_selected = None;
        self.status_message = format!("Removed {} items", removed);
        self.save_config();
    }

    /// Does this item survive the current search box?
    fn matches_filter(&self, item: &VaultItem) -> bool {
        path_matches_filter(&item.original_path, &self.search_filter)
    }

    /// Indices of the items the user can actually see right now.
    ///
    /// Select-all and every bulk action work from this, never from the whole list:
    /// acting on rows hidden behind a search filter is how you encrypt a file you
    /// did not know was selected.
    fn visible_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.matches_filter(item))
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Paths of the current selection, in list order, for confirmation prompts.
    fn selected_paths(&self) -> Vec<PathBuf> {
        let mut indices: Vec<usize> = self.selected.iter().copied().collect();
        indices.sort_unstable();
        indices
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.original_path.clone()))
            .collect()
    }

    /// Re-measure the items a finished operation touched.
    ///
    /// A failed measurement keeps the previous value: after a lock the original file
    /// is gone, and showing the size it had is far more useful than "N/A".
    fn refresh_sizes(&mut self, indices: &[usize]) {
        for &idx in indices {
            if let Some(item) = self.items.get_mut(idx) {
                let probe = item
                    .encrypted_path
                    .as_ref()
                    .filter(|_| item.is_locked)
                    .map(|p| p.as_path())
                    .unwrap_or(item.original_path.as_path());
                if let Some(size) = measure_size(probe) {
                    item.size = Some(size);
                }
            }
        }
    }

    fn scan_locked_files(&mut self, folder: &Path) {
        // Recursively scan a folder for encrypted files and add them to the vault
        let mut found_count = 0;
        let mut skipped_count = 0;
        let mut total_scanned = 0;
        let mut error_count = 0;

        for entry in WalkDir::new(folder)
            .into_iter()
            .filter_map(|e| {
                match e {
                    Ok(entry) => Some(entry),
                    Err(_) => {
                        error_count += 1;
                        None
                    }
                }
            })
            .filter(|e| e.file_type().is_file())
        {
            total_scanned += 1;
            let file_path = entry.path();
            // Check if it's a MyVault encrypted file
            if crate::crypto::is_encrypted_file(file_path) {
                // Get original filename by removing .vault.encrypted suffix
                if let Some(original) = Self::original_path_for(file_path) {
                    // Check if this file is already in the vault
                    let already_added = self.items.iter().any(|item| {
                        item.encrypted_path.as_ref().map(|p| p == file_path).unwrap_or(false)
                    });

                    if already_added {
                        skipped_count += 1;
                    } else {
                        // Add as locked file. The original is gone, so the closest
                        // thing to a size we can show is the ciphertext's.
                        let item = VaultItem {
                            original_path: original,
                            encrypted_path: Some(file_path.to_path_buf()),
                            is_locked: true,
                            item_type: ItemType::File,
                            is_folder_hidden: false,
                            size: measure_size(file_path),
                        };
                        self.items.push(item);
                        found_count += 1;
                    }
                }
            }
        }

        if found_count > 0 {
            self.save_config();
            if skipped_count > 0 || error_count > 0 {
                self.status_message = format!(
                    "Scan complete: Added {} new files, skipped {} duplicates ({} total scanned, {} errors)",
                    found_count, skipped_count, total_scanned, error_count
                );
            } else {
                self.status_message = format!(
                    "Found and added {} locked files ({} total scanned)",
                    found_count, total_scanned
                );
            }
        } else if skipped_count > 0 {
            self.status_message = format!(
                "Scanned {} files - all {} encrypted files already in vault",
                total_scanned, skipped_count
            );
        } else {
            self.status_message = format!(
                "Scanned {} files - no locked files found",
                total_scanned
            );
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
            total: all_files.len(),
            queue: all_files,
            rx: None,
            scanning_done: true,
            processed: 0,
            failures: 0,
            _item_index: *selected_indices.first().unwrap(),
            affected_items: selected_indices.clone(),
            error_details: Vec::new(),
            start_time: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            canceling: false,
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

        // Unhide folders BEFORE collecting files
        let folder_indices: Vec<usize> = selected_indices.iter()
            .filter(|&&idx| {
                self.items.get(idx)
                    .map(|item| item.item_type == ItemType::Folder && item.is_folder_hidden)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if !folder_indices.is_empty() {
            self.last_error_report.clear();
            self.unhide_folders(folder_indices);
        }

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
            total: all_files.len(),
            queue: all_files,
            rx: None,
            scanning_done: true,
            processed: 0,
            failures: 0,
            _item_index: *selected_indices.first().unwrap(),
            affected_items: selected_indices.clone(),
            error_details: Vec::new(),
            start_time: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            canceling: false,
        });

        let file_count = self.current_op.as_ref().unwrap().queue.len();
        self.status_message = format!("Starting unlock: {} files from {} items...", file_count, selected_indices.len());
    }

    fn hide_folders(&mut self, folder_indices: Vec<usize>) {
        let mut hidden_count = 0;
        let mut error_count = 0;

        for idx in folder_indices {
            if let Some(item) = self.items.get_mut(idx) {
                if item.item_type == ItemType::Folder && !item.is_folder_hidden {
                    let folder_path = &item.original_path;

                    // Check if folder exists
                    if !folder_path.exists() {
                        self.last_error_report.push((
                            folder_path.clone(),
                            "Folder does not exist".to_string()
                        ));
                        error_count += 1;
                        continue;
                    }

                    // Check if it's a directory
                    if !folder_path.is_dir() {
                        self.last_error_report.push((
                            folder_path.clone(),
                            "Path is not a directory".to_string()
                        ));
                        error_count += 1;
                        continue;
                    }

                    // Hide the folder
                    match crate::platform::hide(folder_path) {
                        Ok(_) => {
                            item.is_folder_hidden = true;
                            hidden_count += 1;
                        }
                        Err(e) => {
                            self.last_error_report.push((
                                folder_path.clone(),
                                format!("Failed to hide folder: {}", e)
                            ));
                            error_count += 1;
                        }
                    }
                }
            }
        }

        // Save config if any folders were hidden
        if hidden_count > 0 {
            self.save_config();
        }

        // Update status message
        if error_count > 0 {
            self.status_message = format!(
                "Hidden {} folder(s), {} error(s) - click 'View Error Report'",
                hidden_count, error_count
            );
            self.show_error_report = true;
        } else if hidden_count > 0 {
            self.status_message = format!("Hidden {} folder(s)", hidden_count);
        }
    }

    fn unhide_folders(&mut self, folder_indices: Vec<usize>) {
        let mut unhidden_count = 0;
        let mut error_count = 0;

        for idx in folder_indices {
            if let Some(item) = self.items.get_mut(idx) {
                if item.item_type == ItemType::Folder && item.is_folder_hidden {
                    let folder_path = &item.original_path;

                    // Check if folder exists
                    if !folder_path.exists() {
                        // Folder was deleted - just update state
                        item.is_folder_hidden = false;
                        continue;
                    }

                    // Unhide the folder
                    match crate::platform::unhide(folder_path) {
                        Ok(_) => {
                            item.is_folder_hidden = false;
                            unhidden_count += 1;
                        }
                        Err(e) => {
                            self.last_error_report.push((
                                folder_path.clone(),
                                format!("Failed to unhide folder: {}", e)
                            ));
                            error_count += 1;
                        }
                    }
                }
            }
        }

        // Save config if any folders were unhidden
        if unhidden_count > 0 {
            self.save_config();
        }

        // Show errors if any
        if error_count > 0 {
            self.status_message = format!(
                "Unhidden {} folder(s), {} error(s) - click 'View Error Report'",
                unhidden_count, error_count
            );
            self.show_error_report = true;
        }
    }
}

impl eframe::App for MyVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Phase 1: Apply dark mode theme. Only on change - rebuilding the whole
        // palette every frame threw away any styling applied on top of it.
        if self.applied_dark_mode != Some(self.dark_mode) {
            apply_theme(ctx, self.dark_mode);
            self.applied_dark_mode = Some(self.dark_mode);
        }

        // Phase 3: Session timeout check.
        //
        // egui only repaints in response to input, so while the app sat untouched
        // this check never ran - exactly the situation auto-lock exists for. Ask for
        // a repaint every second so the timer keeps ticking (and the countdown in
        // the status bar stays live) with nobody at the keyboard.
        if self.auto_lock_enabled && self.authenticated {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));

            let elapsed_minutes = self.last_activity.elapsed().as_secs() / 60;
            if elapsed_minutes >= self.session_timeout_minutes {
                let msg = format!(
                    "🔒 Session timed out after {} minutes of inactivity. Please re-authenticate.",
                    self.session_timeout_minutes
                );
                self.lock_session(&msg);
            }
        }

        // Phase 3: Update activity timestamp on any user interaction
        if ctx.input(|i| !i.events.is_empty()) {
            self.last_activity = Instant::now();
        }

        // Phase 2: Keyboard shortcuts
        // `modifiers.command` is Cmd on macOS and Ctrl everywhere else. Matching on
        // `.ctrl` meant none of these shortcuts fired in the macOS build.
        // Suppressed while any dialog is open so Escape closes the dialog rather
        // than silently clearing the selection underneath it.
        if self.authenticated && !self.any_dialog_open() {
            // Computed outside the input closure: `visible_indices` borrows self.
            let visible = self.visible_indices();
            ctx.input(|i| {
                // Cmd/Ctrl+A: Select every *visible* row. Selecting rows hidden by
                // the search filter would let a later Lock encrypt files the user
                // never saw.
                if i.modifiers.command && i.key_pressed(egui::Key::A) {
                    self.selected.clear();
                    for idx in &visible {
                        self.selected.insert(*idx);
                    }
                }

                // Cmd/Ctrl+L: Lock selected files
                if i.modifiers.command && i.key_pressed(egui::Key::L) {
                    let has_selection = !self.selected.is_empty();
                    let some_selected_unlocked = self.selected.iter()
                        .any(|&idx| self.items.get(idx).map(|it| !it.is_locked).unwrap_or(false));
                    if has_selection && some_selected_unlocked {
                        self.confirm_action = Some(ConfirmAction::Lock);
                    }
                }

                // Cmd/Ctrl+U: Unlock selected files
                if i.modifiers.command && i.key_pressed(egui::Key::U) {
                    let has_selection = !self.selected.is_empty();
                    let all_selected_locked = !self.selected.is_empty() &&
                        self.selected.iter().all(|&idx| self.items.get(idx).map(|it| it.is_locked).unwrap_or(false));
                    if has_selection && all_selected_locked {
                        self.confirm_action = Some(ConfirmAction::Unlock);
                    }
                }

                // Delete: Remove selected items
                if i.key_pressed(egui::Key::Delete)
                    && !self.selected.is_empty() {
                        self.confirm_action = Some(ConfirmAction::Remove);
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
            // Phase 3: Collect successful paths to add to recent files after retain_mut
            let mut successful_paths = Vec::new();

            self.op_result_rxs.retain_mut(|(_, rx)| {
                match rx.try_recv() {
                    Ok((path, outcome)) => {
                        match outcome {
                            WorkerOutcome::Done => {
                                op.processed += 1;
                                // Phase 3: Store path for later addition to recent files
                                successful_paths.push(path);
                            }
                            WorkerOutcome::Failed(err) => {
                                op.processed += 1;
                                op.failures += 1;
                                op.error_details.push((path, err));
                            }
                            // Cancelled before it touched anything: nothing happened
                            // to this file, so it is neither done nor failed.
                            WorkerOutcome::Skipped => {}
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

            // Phase 3: Add successful paths to recent files (after retain_mut to avoid borrow conflicts)
            for path in successful_paths {
                self.add_to_recent_files(path);
            }

            // Spawn new background threads up to max_parallel limit
            while self.op_result_rxs.len() < max_parallel && !op.queue.is_empty() {
                let Some(p) = op.queue.pop_front() else { break };

                // Each worker gets its own handle to the key; it is wiped when the
                // worker thread finishes.
                let key = match &self.encryption_key {
                    Some(k) => k.clone(),
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
                let p_display = p.clone();
                let cancel = Arc::clone(&op.cancel);
                let _perf_config = self.perf_config.clone();  // Reserved for future adaptive performance tuning
                std::thread::spawn(move || {
                    // Last chance to back out. Once encryption starts we let it
                    // finish: aborting mid-file would leave a truncated ciphertext
                    // next to a deleted original.
                    if cancel.load(Ordering::Relaxed) {
                        let _ = result_tx.send((p_clone, WorkerOutcome::Skipped));
                        return;
                    }

                    let res = match op_kind {
                        BatchOpKind::LockFolder => {
                            let out = MyVaultApp::encrypted_path_for(&p);

                            // Refuse to overwrite an existing encrypted file: File::create
                            // truncates, which would destroy the previous ciphertext.
                            if out.exists() {
                                let _ = result_tx.send((
                                    p_clone,
                                    WorkerOutcome::Failed(format!(
                                        "Encrypted file already exists: {}",
                                        out.display()
                                    )),
                                ));
                                return;
                            }

                            // Determine encryption strategy based on file size
                            let _file_size = std::fs::metadata(&p)
                                .map(|m| m.len())
                                .unwrap_or(0);  // Reserved for future adaptive chunk sizing

                            // Use streaming encryption for all files to prevent memory exhaustion
                            // when processing many files in parallel. Streaming is memory-safe and
                            // provides good performance with the optimized 16MB chunks.
                            let encrypt_result = crate::crypto::encrypt_file_streaming(key.as_bytes(), &p, &out);

                            match encrypt_result {
                                Ok(_) => {
                                    let _ = crate::platform::hide_encrypted_file(&out);
                                    let _ = std::fs::remove_file(&p);
                                    WorkerOutcome::Done
                                }
                                Err(e) => WorkerOutcome::Failed(format!("Encryption failed: {}", e)),
                            }
                        }
                        BatchOpKind::UnlockFolder => {
                            let _ = crate::platform::unhide_encrypted_file(&p);
                            if let Some(out) = MyVaultApp::original_path_for(&p) {
                                // Check if original file already exists
                                if out.exists() {
                                    WorkerOutcome::Failed(format!("Original file exists: {}", out.display()))
                                } else {
                                    // Use streaming decryption for all files to prevent memory exhaustion
                                    // when processing many files in parallel. Streaming is memory-safe and
                                    // provides good performance with optimized chunk sizes.
                                    let decrypt_result = crate::crypto::decrypt_file_streaming(key.as_bytes(), &p, &out);

                                    match decrypt_result {
                                        Ok(_) => {
                                            // Force file handle cleanup
                                            drop(decrypt_result);
                                            let _ = std::fs::remove_file(&p);
                                            WorkerOutcome::Done
                                        }
                                        Err(e) => WorkerOutcome::Failed(format!("Decryption failed: {}", e)),
                                    }
                                }
                            } else {
                                WorkerOutcome::Failed("Invalid encrypted filename".to_string())
                            }
                        }
                    };
                    let _ = result_tx.send((p_clone, res));
                });

                // No artificial delay here: the number of in-flight workers is already
                // bounded by `max_parallel` (<= 4), so file descriptors cannot pile up.
                // Sleeping on the UI thread would freeze the interface for each spawn.
                self.op_result_rxs.push((p_display, result_rx));
            }

            // Complete only when scanning is done, queue is empty, AND all background threads finished
            if op.scanning_done && op.queue.is_empty() && self.op_result_rxs.is_empty() {
                // A cancelled run stopped part-way through, so the items it covers are
                // in a mixed state - claiming they are all locked (or all unlocked)
                // would be a lie. Leave the flags alone and say so in the status bar.
                if !op.canceling {
                    // Update all affected items (not just the first one)
                    for &idx in &op.affected_items {
                        if let Some(item) = self.items.get_mut(idx) {
                            match op.kind {
                                BatchOpKind::LockFolder => item.is_locked = true,
                                BatchOpKind::UnlockFolder => item.is_locked = false,
                            }
                        }
                    }
                }

                let affected = op.affected_items.clone();
                self.refresh_sizes(&affected);

                // Hide folders after successful lock
                if matches!(op.kind, BatchOpKind::LockFolder) && !op.canceling {
                    let folder_indices: Vec<usize> = op.affected_items.iter()
                        .filter(|&&idx| {
                            self.items.get(idx)
                                .map(|item| item.item_type == ItemType::Folder)
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();

                    if !folder_indices.is_empty() {
                        if op.failures == 0 {
                            // All files succeeded - hide immediately
                            self.hide_folders(folder_indices);
                        } else {
                            // Some failures - ask for confirmation
                            self.confirm_action = Some(ConfirmAction::HideFolderWithFailures {
                                folder_indices,
                                total_files: op.processed,
                                failed_files: op.failures,
                            });
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

                let msg = if op.canceling {
                    let verb = match op.kind {
                        BatchOpKind::LockFolder => "lock",
                        BatchOpKind::UnlockFolder => "unlock",
                    };
                    format!(
                        "Canceled {} after {} of {} files ({} errors) in {} - the remaining files were left untouched, run it again to finish",
                        verb, op.processed, op.total, op.failures, time_str
                    )
                } else { match op.kind {
                    BatchOpKind::LockFolder => {
                        let folder_count = op.affected_items.iter()
                            .filter(|&&idx| {
                                self.items.get(idx)
                                    .map(|item| item.item_type == ItemType::Folder)
                                    .unwrap_or(false)
                            })
                            .count();

                        if op.failures == 0 {
                            if folder_count > 0 {
                                format!("Locked and hidden {} folder(s) with {} files in {}",
                                    folder_count, op.processed, time_str)
                            } else {
                                format!("Locked {} items in {}", op.affected_items.len(), time_str)
                            }
                        } else {
                            format!("Locked {} items with {} errors in {} - click 'View Error Report' to see details", op.affected_items.len(), op.failures, time_str)
                        }
                    },
                    BatchOpKind::UnlockFolder => {
                        let folder_count = op.affected_items.iter()
                            .filter(|&&idx| {
                                self.items.get(idx)
                                    .map(|item| item.item_type == ItemType::Folder)
                                    .unwrap_or(false)
                            })
                            .count();

                        if op.failures == 0 {
                            if folder_count > 0 {
                                format!("Unlocked and unhidden {} folder(s) with {} files in {}",
                                    folder_count, op.processed, time_str)
                            } else {
                                format!("Unlocked {} items in {}", op.affected_items.len(), time_str)
                            }
                        } else {
                            format!("Unlocked {} items with {} errors in {} - click 'View Error Report' to see details", op.affected_items.len(), op.failures, time_str)
                        }
                    }
                } };
                self.status_message = msg;
                // completed
            } else {
                self.current_op = Some(op);
                ctx.request_repaint();
            }
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            // Wrapping, so narrowing the window moves buttons to a second line
            // instead of cutting the trailing ones off.
            ui.horizontal_wrapped(|ui| {
                ui.heading("My Vault App");
                ui.separator();

                // One primary control for the vault: unlock it, or lock it again.
                // Changing the password and switching theme moved into Settings -
                // they were competing for space with the things used every session.
                if self.authenticated {
                    if ui.button("🔒 Lock Vault")
                        .on_hover_text("Lock the vault now and clear the key from memory")
                        .clicked()
                    {
                        self.lock_session("🔒 Vault locked");
                    }
                } else {
                    let mp_label = if self.master_password_hash.is_some() {
                        "🔓 Unlock Vault"
                    } else {
                        "Create Master Password"
                    };
                    if ui.button(mp_label).clicked() {
                        self.show_password_dialog = true;
                    }
                }

                ui.separator();
                // Phase 3: Settings button
                if ui.button("⚙ Settings").on_hover_text("Theme, auto-lock, master password").clicked() {
                    self.show_settings_dialog = true;
                }

                // Phase 3: Recent Files dropdown
                ui.menu_button("📂 Recent Files", |ui| {
                    if self.recent_files.is_empty() {
                        ui.label("No recent files");
                    } else {
                        ui.label("Click to add back to list:");
                        ui.separator();

                        let recent_files_clone = self.recent_files.clone();
                        for (idx, path) in recent_files_clone.iter().enumerate() {
                            if idx >= 20 { break; } // Show max 20 files

                            let file_name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown");

                            if ui.button(format!("{}. {}", idx + 1, file_name))
                                .on_hover_text(path.to_string_lossy().as_ref())
                                .clicked()
                            {
                                let item_type = if path.is_dir() { ItemType::Folder } else { ItemType::File };
                                self.add_path(path.clone(), item_type);
                                ui.close();
                            }
                        }
                    }
                });
            });
        });

        // Phase 3: Password change reminder banner
        if self.authenticated && self.show_password_reminder {
            egui::TopBottomPanel::top("password_reminder").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "⚠");

                    let age_str = if let Some(last_changed) = self.password_last_changed {
                        if let Ok(age) = SystemTime::now().duration_since(last_changed) {
                            let days = age.as_secs() / 86400;
                            format!("Your password is {} days old. ", days)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    ui.label(format!("{}Consider changing it for better security.", age_str));

                    if ui.button("Change Now").clicked() {
                        self.show_change_password_dialog = true;
                        self.show_password_reminder = false;
                    }

                    if ui.button("Remind Me in 7 Days").clicked() {
                        // Dismiss reminder for 7 days
                        self.reminder_dismissed_until = Some(
                            SystemTime::now() + std::time::Duration::from_secs(7 * 24 * 60 * 60)
                        );
                        self.show_password_reminder = false;
                        self.save_config();
                    }

                    if ui.button("Don't Remind Me").clicked() {
                        // Dismiss reminder indefinitely
                        self.reminder_dismissed_until = Some(
                            SystemTime::now() + std::time::Duration::from_secs(365 * 24 * 60 * 60) // 1 year
                        );
                        self.show_password_reminder = false;
                        self.save_config();
                    }

                    if ui.button("✖").clicked() {
                        self.show_password_reminder = false;
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Files & folders list");
            ui.separator();

            ui.horizontal(|ui| {
                let busy = self.current_op.is_some();
                if ui.add_enabled(!busy, egui::Button::new("Add Files"))
                    .on_hover_text("Add one or more files to encrypt/decrypt")
                    .clicked() {
                    // Multi-select: adding a folder's worth of files one dialog at
                    // a time was the only way to do this before.
                    if let Some(paths) = rfd::FileDialog::new().pick_files() {
                        for path in paths {
                            self.add_path(path, ItemType::File);
                        }
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
                    .on_hover_text(format!("Encrypt selected files ({}+L)", CMD_LABEL))
                    .clicked() {
                    self.confirm_action = Some(ConfirmAction::Lock);
                }

                let can_unlock = !busy && has_selection && self.authenticated && all_selected_locked;
                if ui.add_enabled(can_unlock, egui::Button::new("Unlock"))
                    .on_hover_text(format!("Decrypt selected files ({}+U)", CMD_LABEL))
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

            // Locked vault: offer the way back in right here, rather than a dead
            // heading that leaves the user hunting the toolbar for the button.
            if !self.authenticated {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.heading("🔒 Vault is locked");
                    ui.add_space(4.0);
                    ui.label("Unlock the vault to see and manage your files.");
                    ui.add_space(12.0);
                    let label = if self.master_password_hash.is_some() {
                        "🔓 Unlock Vault"
                    } else {
                        "Create Master Password"
                    };
                    if ui.button(egui::RichText::new(label).size(16.0)).clicked() {
                        self.show_password_dialog = true;
                    }
                });
                return;
            }

            // Phase 2: Drag and drop support (detect at panel level, before borrowing items)
            if let Some(dropped_files) = ctx.input(|i| {
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

            // Phase 2: Prepare filtered and sorted items
            let mut display_items: Vec<(usize, &VaultItem)> = self.items.iter().enumerate()
                .filter(|(_, item)| self.matches_filter(item))
                .collect();

            // Sort items. Sizes come from the per-item cache: comparing them used to
            // stat both operands, so a single repaint issued O(n log n) syscalls.
            let sort_by = self.sort_by;
            let sort_ascending = self.sort_ascending;
            let compare = move |a: &VaultItem, b: &VaultItem| {
                let ordering = match sort_by {
                    SortField::Name => {
                        a.original_path.file_name().unwrap_or_default()
                            .to_string_lossy()
                            .cmp(&b.original_path.file_name().unwrap_or_default().to_string_lossy())
                    }
                    SortField::Status => {
                        a.is_locked.cmp(&b.is_locked)
                    }
                    SortField::Size => {
                        a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0))
                    }
                };
                if sort_ascending {
                    ordering
                } else {
                    ordering.reverse()
                }
            };
            display_items.sort_by(|(_, a), (_, b)| compare(a, b));

            // Group items by parent directory. The sort above already fixed the order
            // within each group, so there is no second per-group sort here.
            let mut groups: BTreeMap<PathBuf, Vec<(usize, &VaultItem)>> = BTreeMap::new();
            for item in &display_items {
                let parent = item.1.original_path.parent()
                    .unwrap_or(Path::new("/"))
                    .to_path_buf();
                groups.entry(parent).or_default().push(*item);
            }

            // The order rows actually appear in on screen. Shift+click ranges are
            // resolved against this, not against `items` indices - those are insertion
            // order, so a shift-selection used to grab a completely different set of
            // files than the ones highlighted between the two clicks.
            let visual_order: Vec<usize> = groups
                .values()
                .flat_map(|items| items.iter().map(|(idx, _)| *idx))
                .collect();

            // Show message if filtering resulted in empty list
            if display_items.is_empty() && !self.items.is_empty() {
                ui.label("No items match the search filter");
            } else if self.items.is_empty() {
                ui.label("No files added yet. Use buttons above or drag & drop files here.");
            }

            // Flatten the folder groups into one uniform-height row list. This is what
            // lets the scroll area skip rows that are off-screen: previously every row
            // of the vault was laid out on every frame, whether or not it was visible.
            let rows = flatten_rows(&groups);

            // Must match what `selectable_label` actually allocates, or the scroll
            // area's virtual height drifts away from the real content.
            let row_height = (ui.text_style_height(&egui::TextStyle::Body)
                + 2.0 * ui.spacing().button_padding.y)
                .max(ui.spacing().interact_size.y);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show_rows(ui, row_height, rows.len(), |ui, range| {
                    ui.set_width(ui.available_width());

                    for row in &rows[range] {
                        match row {
                            ListRow::Header { dir, members } => {
                                // Clicking a folder header selects or clears everything under it
                                let all_selected = !members.is_empty()
                                    && members.iter().all(|(idx, _)| self.selected.contains(idx));
                                let label =
                                    egui::RichText::new(format!("📁 {}", dir.display())).strong();

                                if ui.selectable_label(all_selected, label).clicked() {
                                    for (idx, _) in members.iter() {
                                        if all_selected {
                                            self.selected.remove(idx);
                                        } else {
                                            self.selected.insert(*idx);
                                        }
                                    }
                                }
                            }

                            ListRow::Item { idx, item } => {
                                let idx = *idx;
                                let is_selected = self.selected.contains(&idx);

                                // Multi-select with Cmd/Ctrl+click and Shift+click for range selection
                                if file_row(ui, item, is_selected, row_height).clicked() {
                                    let modifiers = ui.ctx().input(|i| i.modifiers);
                                    if modifiers.shift {
                                        // Range select with Shift held, walking the rows as
                                        // they are drawn so the selection matches what the
                                        // user sees highlighted.
                                        for v in shift_range(&visual_order, self.last_selected, idx) {
                                            self.selected.insert(v);
                                        }
                                        self.last_selected = Some(idx);
                                    } else if modifiers.command {
                                        // Toggle with Cmd/Ctrl held
                                        if is_selected {
                                            self.selected.remove(&idx);
                                        } else {
                                            self.selected.insert(idx);
                                        }
                                        self.last_selected = Some(idx);
                                    } else {
                                        // Single select without modifiers
                                        self.selected.clear();
                                        self.selected.insert(idx);
                                        self.last_selected = Some(idx);
                                    }
                                }
                            }
                        }
                    }
            });

        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Whether the vault is open, and how long it stays open, were both
                // invisible - the only clue was whether the file list was greyed out.
                if self.authenticated {
                    // An open vault is the state worth noticing, hence Warn, not Good:
                    // "unlocked" is exactly when the files are exposed.
                    ui.colored_label(tone_color(ui, Tone::Warn), "🔓 Unlocked");
                    if let Some(remaining) = self.auto_lock_remaining() {
                        // Warn once the window is short enough to matter.
                        let color = if remaining <= 60 {
                            tone_color(ui, Tone::Bad)
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        ui.colored_label(
                            color,
                            format!("· auto-lock in {}:{:02}", remaining / 60, remaining % 60),
                        )
                        .on_hover_text("Any activity resets this timer. Configure it in Settings.");
                    } else {
                        ui.colored_label(
                            ui.visuals().weak_text_color(),
                            "· auto-lock off",
                        );
                    }
                } else {
                    ui.colored_label(tone_color(ui, Tone::Good), "🔒 Locked");
                }
                ui.separator();

                let msg = self.status_message.as_str();
                let err = msg.contains("error") || msg.contains("Error") || msg.contains("Invalid") || msg.contains("failed") || msg.contains("Failed");
                let ok = msg.contains("Locked") || msg.contains("Unlocked") || msg.contains("Authenticated") || msg.contains("Loaded") || msg.contains("Added") || msg.contains("created") || msg.contains("Removed");
                let color = if err {
                    tone_color(ui, Tone::Bad)
                } else if ok {
                    tone_color(ui, Tone::Good)
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

        // Password dialog. A real modal: it dims and blocks the rest of the UI,
        // and Escape or a click outside dismisses it.
        if self.show_password_dialog {
            let has_hash = self.master_password_hash.is_some();
            let title = if has_hash { "Enter Master Password" } else { "Create Master Password" };
            let modal = egui::Modal::new(egui::Id::new("password_dialog"))
                .show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.heading(title);
                    ui.separator();
                    ui.vertical(|ui| {
                        // Track if Enter key was pressed for auto-submit
                        let mut enter_pressed = false;

                        if has_hash {
                            ui.label("Enter your master password:");
                            let resp = password_field(
                                ui,
                                &mut self.temp_password,
                                "Password",
                                &mut self.reveal_passwords,
                            );
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                enter_pressed = true;
                            }
                        } else {
                            ui.label("Create a master password (store it safely):");
                            password_field(
                                ui,
                                &mut self.temp_password,
                                "Password",
                                &mut self.reveal_passwords,
                            );

                            // Phase 1: Password strength meter
                            strength_meter(ui, &self.temp_password, 150.0);

                            let confirm_resp = password_field(
                                ui,
                                &mut self.temp_password_confirm,
                                "Confirm password",
                                &mut self.reveal_passwords,
                            );
                            // Auto-submit on Enter in confirm field
                            if confirm_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                enter_pressed = true;
                            }
                        }

                        ui.horizontal(|ui| {
                            // Phase 3: Password generator button (only for password creation, not authentication)
                            if !has_hash
                                && ui.button("🔐 Generate").on_hover_text("Open password generator").clicked() {
                                    self.show_password_generator = true;
                                }

                            if ui.button("Cancel").clicked() {
                                self.close_password_dialog();
                            }

                            if has_hash {
                                // Trigger authentication on button click OR Enter key
                                if ui.button("Enter").clicked() || enter_pressed {
                                    if let Some(hash) = self.master_password_hash.as_deref() { match crypto::verify_password(&self.temp_password, hash) {
                                        Ok(true) => {
                                            // Recover the Data Encryption Key for this session
                                            match self.unlock_vault_key(&self.temp_password.clone()) {
                                                Ok(()) => {
                                                    self.authenticated = true;
                                                    self.status_message = "Authenticated".to_string();
                                                    self.show_password_dialog = false;

                                                    // Phase 3: Check if password change reminder should be shown
                                                    self.check_password_reminder();
                                                }
                                                Err(e) => {
                                                    self.status_message = format!("Could not unlock vault: {}", e);
                                                }
                                            }
                                        }
                                        Ok(false) => {
                                            self.status_message = "Invalid password".to_string();
                                        }
                                        Err(e) => {
                                            self.status_message = format!("Password verification error: {}", e);
                                        }
                                    } }
                                    self.temp_password.zeroize();
                                    self.temp_password.clear();
                                }
                            } else {
                                // Trigger password creation on button click OR Enter key
                                if ui.button("Create").clicked() || enter_pressed {
                                    if self.temp_password.is_empty() {
                                        self.status_message = "Password cannot be empty".to_string();
                                    } else if self.temp_password != self.temp_password_confirm {
                                        self.status_message = "Passwords do not match".to_string();
                                    } else {
                                        match crypto::hash_password(&self.temp_password) {
                                            Ok((hash, salt)) => {
                                                // Generate and wrap the vault's Data Encryption Key
                                                match self.create_vault_key(&self.temp_password.clone(), &salt) {
                                                    Ok(()) => {
                                                        self.master_password_hash = Some(hash);
                                                        self.salt = Some(salt);
                                                        self.authenticated = true;
                                                        self.status_message = "Master password created".to_string();

                                                        // Phase 3: Record password creation time
                                                        self.password_last_changed = Some(SystemTime::now());

                                                        self.save_config();
                                                        self.show_password_dialog = false;
                                                    }
                                                    Err(e) => {
                                                        self.status_message = format!("Key setup error: {}", e);
                                                    }
                                                }
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
            if modal.should_close() {
                self.close_password_dialog();
            }
        }

        // Change password dialog (only show when authenticated)
        if self.show_change_password_dialog && self.authenticated {
            let modal = egui::Modal::new(egui::Id::new("change_password_dialog"))
                .show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.heading("Change Master Password");
                    ui.separator();

                    // Track if Enter key was pressed for auto-submit
                    let mut enter_pressed = false;

                    ui.label("Current password:");
                    password_field(
                        ui,
                        &mut self.current_password,
                        "Current password",
                        &mut self.reveal_passwords,
                    );

                    ui.separator();

                    ui.label("New password:");
                    password_field(
                        ui,
                        &mut self.new_password,
                        "New password",
                        &mut self.reveal_passwords,
                    );

                    // Phase 1: Password strength meter for new password
                    strength_meter(ui, &self.new_password, 150.0);

                    ui.label("Confirm new password:");
                    let confirm_resp = password_field(
                        ui,
                        &mut self.new_password_confirm,
                        "Confirm new password",
                        &mut self.reveal_passwords,
                    );
                    // Auto-submit on Enter in confirm field
                    if confirm_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        enter_pressed = true;
                    }

                    ui.horizontal(|ui| {
                        // Phase 3: Password generator button
                        if ui.button("🔐 Generate").on_hover_text("Open password generator").clicked() {
                            // Open generator (it will populate new_password fields when "Use This Password" is clicked)
                            self.show_password_generator = true;
                        }

                        if ui.button("Cancel").clicked() {
                            self.close_change_password_dialog();
                        }

                        // Trigger password change on button click OR Enter key
                        if ui.button("Change Password").clicked() || enter_pressed {
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
                                                    // Re-wrap the existing Data Encryption Key with the new
                                                    // password. The DEK itself is unchanged, so files that were
                                                    // encrypted under the old password stay decryptable.
                                                    // Only commit the new hash/salt once this succeeds.
                                                    match self.rewrap_vault_key(&self.new_password.clone(), &new_salt) {
                                                        Ok(()) => {
                                                            self.master_password_hash = Some(new_hash);
                                                            self.salt = Some(new_salt);

                                                            // Phase 3: Record password change time and hide reminder
                                                            self.password_last_changed = Some(SystemTime::now());
                                                            self.show_password_reminder = false;
                                                            self.reminder_dismissed_until = None;

                                                            self.save_config();
                                                            self.status_message = "Master password changed successfully".to_string();
                                                            self.show_change_password_dialog = false;
                                                        }
                                                        Err(e) => {
                                                            self.status_message = format!("Password not changed: {}", e);
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
            if modal.should_close() {
                self.close_change_password_dialog();
            }
        }

        // Phase 3: Settings dialog
        if self.show_settings_dialog {
            let modal = egui::Modal::new(egui::Id::new("settings_dialog"))
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("⚙ Settings");
                    ui.separator();

                    // Appearance and the master password moved here from the toolbar,
                    // which was competing for width with the everyday controls.
                    ui.label("Appearance:");
                    let theme_label = if self.dark_mode { "☀ Switch to Light Mode" } else { "🌙 Switch to Dark Mode" };
                    if ui.button(theme_label).clicked() {
                        self.dark_mode = !self.dark_mode;
                    }

                    if self.master_password_hash.is_some() && self.authenticated
                        && ui.button("🔑 Change Master Password").clicked() {
                            self.show_change_password_dialog = true;
                            self.show_settings_dialog = false;
                            self.save_config();
                        }

                    ui.separator();
                    ui.heading("Security Settings");
                    ui.separator();

                    // Session Timeout Settings
                    ui.label("Auto-Lock:");
                    ui.checkbox(&mut self.auto_lock_enabled, "Enable session timeout");

                    ui.add_enabled(
                        self.auto_lock_enabled,
                        egui::Slider::new(&mut self.session_timeout_minutes, 1..=60)
                            .text("minutes")
                            .suffix(" min")
                    );

                    if self.auto_lock_enabled {
                        ui.label(format!("⏱ App will auto-lock after {} minutes of inactivity", self.session_timeout_minutes));
                    } else {
                        ui.label("⚠ Auto-lock disabled (not recommended)");
                    }

                    ui.separator();

                    // Password Change Reminder Settings
                    ui.label("Password Change Reminder:");
                    ui.add(
                        egui::Slider::new(&mut self.password_change_reminder_days, 30..=365)
                            .text("days")
                            .suffix(" days")
                    );
                    ui.label(format!("📅 Remind me to change password every {} days", self.password_change_reminder_days));

                    ui.separator();

                    // Manual Lock button
                    if self.authenticated
                        && ui.button("🔒 Lock Now").clicked() {
                            self.show_settings_dialog = false;
                            self.save_config();
                            self.lock_session("🔒 Session locked manually");
                        }

                    ui.separator();

                    // Close button
                    if ui.button("Close").clicked() {
                        self.show_settings_dialog = false;
                        self.save_config(); // Save settings
                    }
                });
            if modal.should_close() {
                self.show_settings_dialog = false;
                self.save_config();
            }
        }

        // Phase 3: Password Generator dialog
        if self.show_password_generator {
            let modal = egui::Modal::new(egui::Id::new("password_generator"))
                .show(ctx, |ui| {
                    ui.set_width(400.0);
                    ui.heading("🔐 Generate Secure Password");
                    ui.separator();

                    // Length slider
                    ui.label("Password Length:");
                    ui.add(egui::Slider::new(&mut self.gen_length, 8..=128).text("characters"));

                    ui.separator();

                    // Character type checkboxes
                    ui.label("Include:");
                    ui.checkbox(&mut self.gen_use_lowercase, "Lowercase (a-z)");
                    ui.checkbox(&mut self.gen_use_uppercase, "Uppercase (A-Z)");
                    ui.checkbox(&mut self.gen_use_digits, "Digits (0-9)");
                    ui.checkbox(&mut self.gen_use_symbols, "Symbols (!@#$%^&*...)");

                    ui.separator();

                    // Generate button
                    if ui.button("🎲 Generate").clicked() {
                        self.generated_password = self.generate_password();
                    }

                    // Display generated password
                    if !self.generated_password.is_empty() {
                        ui.label("Generated Password:");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.generated_password)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(300.0)
                            );

                            // Copy to clipboard button
                            if ui.button("📋 Copy").clicked() {
                                if let Err(e) = crate::platform::set_clipboard(&self.generated_password) {
                                    self.status_message = format!("Failed to copy: {}", e);
                                } else {
                                    self.status_message = "Password copied to clipboard!".to_string();
                                }
                            }
                        });

                        // Password strength indicator. This one used to divide the
                        // level by 100, so the bar never filled past a sliver.
                        strength_meter(ui, &self.generated_password, 200.0);
                    }

                    ui.separator();

                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("Use This Password").clicked() && !self.generated_password.is_empty() {
                            // Determine which dialog is open and copy to appropriate fields
                            if self.show_change_password_dialog {
                                // Copy to change password dialog fields
                                self.new_password = self.generated_password.clone();
                                self.new_password_confirm = self.generated_password.clone();
                            } else {
                                // Copy to create/enter password dialog fields
                                self.temp_password = self.generated_password.clone();
                                self.temp_password_confirm = self.generated_password.clone();
                            }
                            self.show_password_generator = false;
                        }

                        if ui.button("Close").clicked() {
                            self.show_password_generator = false;
                        }
                    });
                });
            if modal.should_close() {
                self.show_password_generator = false;
            }
        }

        // Confirmation dialog for lock/unlock/remove/overwrite
        if !self.show_password_dialog && !self.show_change_password_dialog {
            if let Some(action) = self.confirm_action.clone() {
                let title = match action {
                    ConfirmAction::Lock => "Confirm Lock",
                    ConfirmAction::Unlock => "Confirm Unlock",
                    ConfirmAction::Remove => "Confirm Remove",
                    ConfirmAction::HideFolderWithFailures { .. } => "Hide Folders with Failures",
                };
                let modal = egui::Modal::new(egui::Id::new("confirm_dialog"))
                    .show(ctx, |ui| {
                        ui.set_width(460.0);
                        ui.heading(title);
                        ui.separator();

                        match action {
                            ConfirmAction::HideFolderWithFailures { ref folder_indices, total_files, failed_files } => {
                                ui.label(format!(
                                    "⚠️ {} out of {} files failed to encrypt.",
                                    failed_files, total_files
                                ));
                                ui.label("Do you still want to hide the folder(s)?");
                                ui.label("Hidden folders with failed files may cause confusion.");

                                ui.horizontal(|ui| {
                                    if ui.button("Hide Folder Anyway").clicked() {
                                        self.hide_folders(folder_indices.clone());
                                        self.confirm_action = None;
                                    }
                                    if ui.button("Keep Folder Visible").clicked() {
                                        self.confirm_action = None;
                                    }
                                });
                            }
                            _ => {
                                // Every one of these acts on the WHOLE selection. The
                                // dialog used to name a single arbitrary item pulled
                                // out of a HashSet, so confirming a 50-file lock looked
                                // like confirming one file - while the originals of all
                                // 50 were deleted.
                                let paths = self.selected_paths();
                                let count = paths.len();
                                let noun = if count == 1 { "item" } else { "items" };

                                match action {
                                    ConfirmAction::Lock => {
                                        ui.label(format!("Encrypt {} selected {}.", count, noun));
                                        ui.colored_label(
                                            egui::Color32::from_rgb(220, 90, 40),
                                            "⚠ The original files are deleted once they are encrypted. Folders are hidden as well.",
                                        );
                                    }
                                    ConfirmAction::Unlock => {
                                        ui.label(format!("Decrypt {} selected {}.", count, noun));
                                        ui.label("The encrypted copies are removed once the originals are restored.");
                                    }
                                    ConfirmAction::Remove => {
                                        ui.label(format!("Remove {} selected {} from the list.", count, noun));
                                        ui.label("Nothing on disk is deleted - encrypted files stay encrypted.");
                                    }
                                    ConfirmAction::HideFolderWithFailures { .. } => unreachable!(),
                                }

                                ui.separator();

                                // Name what is actually affected, capped so a big
                                // selection cannot push the buttons off-screen.
                                const PREVIEW: usize = 6;
                                for path in paths.iter().take(PREVIEW) {
                                    ui.label(format!("• {}", path.display()));
                                }
                                if count > PREVIEW {
                                    ui.label(format!("...and {} more", count - PREVIEW));
                                }

                                ui.separator();
                                ui.horizontal(|ui| {
                                    if ui.button("Cancel").clicked() {
                                        self.confirm_action = None;
                                    }
                                    let confirm_label = match action {
                                        ConfirmAction::Lock => format!("Lock {} {}", count, noun),
                                        ConfirmAction::Unlock => format!("Unlock {} {}", count, noun),
                                        ConfirmAction::Remove => format!("Remove {} {}", count, noun),
                                        ConfirmAction::HideFolderWithFailures { .. } => unreachable!(),
                                    };
                                    if ui.button(confirm_label).clicked() {
                                        match action {
                                            ConfirmAction::Lock => self.lock_selected(),
                                            ConfirmAction::Unlock => self.unlock_selected(),
                                            ConfirmAction::Remove => self.remove_selected(),
                                            ConfirmAction::HideFolderWithFailures { .. } => unreachable!(),
                                        }
                                        self.confirm_action = None;
                                    }
                                });
                            }
                        }
                    });
                // Escape or a click outside means "do not do it" - the safe default
                // for every one of these actions.
                if modal.should_close() {
                    self.confirm_action = None;
                }
            }
        }

        // Error report window
        let mut close_error_report = false;
        if self.show_error_report && !self.last_error_report.is_empty() {
            let error_count = self.last_error_report.len();
            let errors = self.last_error_report.clone();

            let modal = egui::Modal::new(egui::Id::new("error_report"))
                .show(ctx, |ui| {
                    ui.set_width(640.0);
                    ui.heading("Error Report");
                    ui.separator();
                    ui.label(format!("Failed files: {} total errors", error_count));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .max_height(360.0)
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
            if modal.should_close() {
                close_error_report = true;
            }
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
            let total = op.total;
            let canceling = op.canceling;
            let in_flight = self.op_result_rxs.len();

            // Phase 1: Calculate throughput and ETA
            let elapsed = start_time.elapsed().as_secs_f32();
            let throughput = if elapsed > 0.0 && processed > 0 {
                processed as f32 / elapsed
            } else {
                0.0
            };

            let (progress, text, eta_text) = if scanning_done {
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
            // Names of the files being worked on right now. A single huge file used to
            // sit at 0% with no indication anything was happening.
            let in_progress: Vec<String> = self
                .op_result_rxs
                .iter()
                .map(|(path, _)| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();

            let mut cancel_clicked = false;
            // Deliberately not dismissible with Escape: an operation is in flight and
            // the only correct way out is the Cancel button.
            egui::Modal::new(egui::Id::new("batch_progress"))
                .show(ctx, |ui| {
                    ui.set_width(460.0);
                    ui.heading(title);
                    ui.separator();
                    ui.label(&text);
                    if !eta_text.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(100, 149, 237), &eta_text);
                    }
                    if scanning_done {
                        ui.add(egui::widgets::ProgressBar::new(progress).show_percentage());
                    } else {
                        ui.add(egui::Spinner::new());
                    }

                    if !in_progress.is_empty() {
                        ui.add_space(4.0);
                        let verb = match kind {
                            BatchOpKind::LockFolder => "Encrypting",
                            BatchOpKind::UnlockFolder => "Decrypting",
                        };
                        for name in &in_progress {
                            ui.label(
                                egui::RichText::new(format!("{} {}", verb, name))
                                    .weak()
                                    .small(),
                            );
                        }
                    }

                    if canceling {
                        // Be honest about what cancelling can and cannot do: the files
                        // already being encrypted have to finish, or they would be left
                        // truncated.
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 170, 60),
                                format!(
                                    "Canceling - finishing {} file(s) already in progress...",
                                    in_flight
                                ),
                            );
                        });
                        ui.label(format!("{} file(s) will be skipped.", queue_len));
                    } else if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            if cancel_clicked {
                // Cancelling used to just drop the operation. The worker threads kept
                // encrypting and deleting files in the background, and their receivers
                // stayed in `op_result_rxs`, so the *next* operation inherited them and
                // reported nonsense. Instead: stop dispatching, let the in-flight
                // workers report back, and finish through the normal completion path.
                if let Some(op) = self.current_op.as_mut() {
                    op.cancel.store(true, Ordering::Relaxed);
                    op.canceling = true;
                    op.queue.clear();
                }
                ctx.request_repaint();
            }
        }
    }
}

/// Semantic meaning of a piece of coloured text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    /// Something is safe or succeeded. Encrypted files are Good, not Bad -
    /// a locked file is the desired state, not an error.
    Good,
    /// Something needs attention but is not broken: a plaintext file, a
    /// middling password, a session about to expire.
    Warn,
    /// A failure.
    Bad,
}

/// Resolve a tone to a colour that is legible on the current background.
///
/// `Color32::GREEN` and `Color32::RED` are pure #00FF00 / #FF0000 - they glare on
/// a dark background and wash out to near-invisible on a light one. These are
/// picked per theme instead.
fn tone_color(ui: &egui::Ui, tone: Tone) -> egui::Color32 {
    let dark = ui.visuals().dark_mode;
    match (tone, dark) {
        (Tone::Good, false) => egui::Color32::from_rgb(21, 115, 71),
        (Tone::Good, true) => egui::Color32::from_rgb(94, 200, 140),
        (Tone::Warn, false) => egui::Color32::from_rgb(150, 90, 10),
        (Tone::Warn, true) => egui::Color32::from_rgb(230, 170, 60),
        (Tone::Bad, false) => egui::Color32::from_rgb(176, 42, 42),
        (Tone::Bad, true) => egui::Color32::from_rgb(240, 115, 115),
    }
}

/// How much of the password-strength bar to fill, for a level of 0..=2.
///
/// Shared by all three strength meters. One of them used to divide the level by
/// 100 instead, so its bar sat at 2% however strong the password was.
fn strength_fill(level: u8) -> f32 {
    ((level.min(2) as f32) + 1.0) / 3.0
}

/// Case-insensitive substring match of a path against the search box.
/// An empty filter matches everything.
fn path_matches_filter(path: &Path, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    path.to_string_lossy()
        .to_lowercase()
        .contains(&filter.to_lowercase())
}

/// Resolve a Shift+click into the rows between the anchor and the clicked row,
/// **in on-screen order**.
///
/// `visual_order` is the sequence of item indices as they are painted (filtered,
/// sorted, grouped by folder). Ranging over raw item indices instead selects
/// whatever happens to sit between them in insertion order - a different set of
/// files than the ones highlighted on screen.
///
/// If either end is not currently visible, only the clicked row is selected.
fn shift_range(visual_order: &[usize], anchor: Option<usize>, clicked: usize) -> Vec<usize> {
    let anchor_pos = anchor.and_then(|a| visual_order.iter().position(|v| *v == a));
    let clicked_pos = visual_order.iter().position(|v| *v == clicked);

    match (anchor_pos, clicked_pos) {
        (Some(a), Some(c)) => visual_order[a.min(c)..=a.max(c)].to_vec(),
        _ => vec![clicked],
    }
}

/// Read a path's size on disk. The only place that touches the filesystem for
/// sizes - callers cache the result on the item, never re-measure while painting.
fn measure_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

/// Phase 2: Render a cached size in human-readable form.
fn format_file_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "N/A".to_string();
    };
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Phase 1: Password strength assessment
/// Returns (strength_level, color, label)
/// - Level 0 (Weak): < 8 chars or simple patterns
/// - Level 1 (Medium): 8-11 chars with some complexity
/// - Level 2 (Strong): 12+ chars with high complexity
fn assess_password_strength(password: &str) -> (u8, Tone, &'static str) {
    if password.is_empty() {
        return (0, Tone::Bad, "");
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
        (0, Tone::Bad, "Weak")
    } else if len >= 12 && complexity >= 3 {
        (2, Tone::Good, "Strong")
    } else if len >= 8 && complexity >= 2 {
        (1, Tone::Warn, "Medium")
    } else {
        (0, Tone::Bad, "Weak")
    }
}

/// The password-strength meter: label, bar and verdict.
///
/// One widget for all three dialogs. They each had their own copy of the drawing
/// code, which is how one of them ended up with a bar that never filled and all
/// three ended up with a hard-coded dark bar track that looked wrong in light mode.
fn strength_meter(ui: &mut egui::Ui, password: &str, bar_width: f32) {
    if password.is_empty() {
        return;
    }
    let (level, tone, label) = assess_password_strength(password);
    let color = tone_color(ui, tone);

    ui.horizontal(|ui| {
        ui.label("Strength:");

        let bar_height = 8.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(bar_width, bar_height),
            egui::Sense::hover(),
        );

        // Track colour comes from the theme so it works in both modes.
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);

        let filled = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(bar_width * strength_fill(level), bar_height),
        );
        ui.painter().rect_filled(filled, 3.0, color);

        ui.colored_label(color, label);
    });
}

/// A password field with a reveal toggle.
///
/// Typing a long generated password blind, twice, with no way to check it, was
/// the previous experience.
fn password_field(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    reveal: &mut bool,
) -> egui::Response {
    let mut response = None;
    ui.horizontal(|ui| {
        response = Some(ui.add(
            egui::TextEdit::singleline(value)
                .password(!*reveal)
                .hint_text(hint)
                .desired_width(ui.available_width() - 34.0),
        ));
        let (icon, tip) = if *reveal {
            ("🔓", "Hide password")
        } else {
            ("👁", "Show password")
        };
        if ui.small_button(icon).on_hover_text(tip).clicked() {
            *reveal = !*reveal;
        }
    });
    response.expect("the horizontal layout always runs")
}

#[derive(Debug)]
struct BatchOp {
    kind: BatchOpKind,
    queue: VecDeque<PathBuf>,
    rx: Option<Receiver<PathBuf>>,
    scanning_done: bool,
    /// Files this operation set out to process, fixed when it starts. Progress is
    /// measured against this rather than against dispatched-so-far, which counted
    /// files that had only been handed to a worker.
    total: usize,
    processed: usize,   // Files that finished, successfully or not
    failures: usize,
    _item_index: usize,  // Reserved for future use
    affected_items: Vec<usize>,  // All item indices involved in this batch operation
    error_details: Vec<(PathBuf, String)>,  // Detailed error tracking: (file_path, error_reason)
    start_time: Instant,  // Track operation start time
    /// Set by the Cancel button. Workers that have not started yet see it and bail
    /// out; ones already encrypting a file run to completion so no file is left
    /// half-written.
    cancel: Arc<AtomicBool>,
    canceling: bool,
}

/// One painted line of the file list.
///
/// The list is grouped by folder, but the scroll area needs a flat sequence of
/// equal-height rows to be able to skip the ones that are off-screen.
enum ListRow<'a> {
    /// A folder heading. Clicking it selects or clears every member below it.
    Header {
        dir: &'a Path,
        members: &'a [(usize, &'a VaultItem)],
    },
    /// One vault item, drawn under the heading above it.
    Item { idx: usize, item: &'a VaultItem },
}

/// Draw one file row: icon, name, size, lock state - in fixed columns.
///
/// The row used to be a single string padded with spaces, so the size and status
/// columns wandered left and right with the length of each filename. Here the
/// whole row is one clickable area with the pieces painted at fixed offsets, which
/// also keeps every row exactly `row_height` tall - the scroll area depends on that
/// to skip the rows that are off screen.
fn file_row(
    ui: &mut egui::Ui,
    item: &VaultItem,
    selected: bool,
    row_height: f32,
) -> egui::Response {
    const INDENT: f32 = 18.0; // sits under its folder heading
    const PAD: f32 = 8.0;
    const ICON_W: f32 = 24.0;
    const SIZE_W: f32 = 90.0;
    const STATUS_W: f32 = 104.0;

    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, row_height), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);

        if selected || response.hovered() {
            ui.painter()
                .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
        }

        let font = egui::TextStyle::Body.resolve(ui.style());
        let text_color = visuals.text_color();

        // Locked means "protected", so it reads as good. Unlocked is the state
        // that deserves attention - the previous colouring had this backwards,
        // painting every encrypted file in error red.
        let state_tone = if item.is_locked { Tone::Good } else { Tone::Warn };
        let state_color = tone_color(ui, state_tone);

        let icon = match item.item_type {
            ItemType::File => "📄",
            ItemType::Folder => "📁",
        };
        let icon_x = rect.left() + INDENT + PAD;
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            icon,
            font.clone(),
            text_color,
        );

        // The name gets whatever is left, and is clipped rather than allowed to
        // run underneath the columns to its right.
        let name_x = icon_x + ICON_W;
        let name_right = rect.right() - PAD - STATUS_W - SIZE_W;
        let name_rect =
            egui::Rect::from_x_y_ranges(name_x..=name_right.max(name_x), rect.y_range());
        let name = item
            .original_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        ui.painter().with_clip_rect(name_rect).text(
            egui::pos2(name_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            font.clone(),
            text_color,
        );

        // Sizes right-align so the digits line up down the column.
        ui.painter().text(
            egui::pos2(rect.right() - PAD - STATUS_W, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            format_file_size(item.size),
            font.clone(),
            ui.visuals().weak_text_color(),
        );

        let state = if item.is_locked { "🔒 Locked" } else { "🔓 Unlocked" };
        ui.painter().text(
            egui::pos2(rect.right() - PAD, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            state,
            font,
            state_color,
        );
    }

    response
}

/// Flatten the folder groups into the exact sequence of rows the list paints.
///
/// The scroll area is told how many rows exist and then asked to draw only a slice
/// of them, so this count has to match what the draw loop emits one-for-one. If it
/// drifts, the scrollbar lies and rows go missing at the bottom of the list.
fn flatten_rows<'a>(
    groups: &'a BTreeMap<PathBuf, Vec<(usize, &'a VaultItem)>>,
) -> Vec<ListRow<'a>> {
    groups
        .iter()
        .flat_map(|(dir, items)| {
            std::iter::once(ListRow::Header {
                dir: dir.as_path(),
                members: items.as_slice(),
            })
            .chain(items.iter().map(|(idx, item)| ListRow::Item { idx: *idx, item }))
        })
        .collect()
}

/// What a worker thread did with one file.
#[derive(Debug)]
enum WorkerOutcome {
    Done,
    Failed(String),
    /// Cancelled before any work started - not a failure, and not progress either.
    Skipped,
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
    HideFolderWithFailures {
        folder_indices: Vec<usize>,
        total_files: usize,
        failed_files: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encrypted-name logic is platform-dependent (a leading dot hides the
    /// file on Unix) and is the only thing that maps a locked file back to its
    /// original name. A mismatch here silently makes files impossible to unlock.
    fn assert_roundtrip(original: &str) {
        let path = PathBuf::from(original);
        let encrypted = MyVaultApp::encrypted_path_for(&path);

        assert_ne!(encrypted, path, "Encrypted name must differ from the original");
        assert!(
            encrypted
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(MyVaultApp::encrypted_suffix()),
            "Encrypted name must carry the vault suffix"
        );

        let recovered = MyVaultApp::original_path_for(&encrypted)
            .unwrap_or_else(|| panic!("Could not map {} back to its original name", original));
        assert_eq!(recovered, path, "Round-trip must recover the original path");
    }

    #[test]
    fn test_path_roundtrip_plain_name() {
        assert_roundtrip("report.pdf");
    }

    #[test]
    fn test_path_roundtrip_in_directory() {
        assert_roundtrip("/tmp/vault/report.pdf");
    }

    #[test]
    fn test_path_roundtrip_no_extension() {
        assert_roundtrip("/tmp/vault/README");
    }

    #[test]
    fn test_path_roundtrip_multiple_dots() {
        assert_roundtrip("/tmp/vault/archive.tar.gz");
    }

    /// Files that already start with a dot are the tricky case: the Unix branch
    /// adds one more dot and must remove exactly one on the way back.
    #[test]
    fn test_path_roundtrip_dotfile() {
        assert_roundtrip("/tmp/vault/.env");
    }

    /// `platform::hide_encrypted_file` is deliberately a no-op on Unix because the
    /// encrypted name already starts with a dot, which is what hides it there.
    /// If that dot ever stops being added, this test fails and points at the
    /// no-op as the thing that has to change with it.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_encrypted_files_are_dot_hidden_on_unix() {
        for original in ["report.pdf", "/tmp/vault/archive.tar.gz", "/tmp/vault/README"] {
            let encrypted = MyVaultApp::encrypted_path_for(&PathBuf::from(original));
            let name = encrypted.file_name().unwrap().to_string_lossy().to_string();
            assert!(
                name.starts_with('.'),
                "{} encrypts to {}, which is not hidden on Unix",
                original,
                name
            );
        }

        // Given the above, hiding an encrypted file needs no extra syscall.
        let tmp = std::env::temp_dir().join(".myvault_hide_noop_test");
        std::fs::write(&tmp, b"x").unwrap();
        assert!(crate::platform::hide_encrypted_file(&tmp).is_ok());
        assert!(tmp.exists(), "The no-op must leave the file untouched");
        let _ = std::fs::remove_file(&tmp);
    }

    /// Folders keep their original name, so on Linux there is no way to hide one.
    /// It must report that rather than claiming success.
    #[cfg(target_os = "linux")]
    #[test]
    fn test_folder_hiding_reports_unsupported_on_linux() {
        let dir = std::env::temp_dir();
        let err = crate::platform::hide(&dir).expect_err("Linux cannot hide a folder");
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
        // Undoing something that never happened is still fine.
        assert!(crate::platform::unhide(&dir).is_ok());
    }

    /// Shift+click must follow what is on screen. The list is filtered, sorted and
    /// grouped by folder, so the painted order has nothing to do with the order
    /// items were added in - which is what the raw indices encode.
    #[test]
    fn test_shift_range_follows_visual_order_not_item_indices() {
        // Rows are painted as items 4, 0, 2 - e.g. after sorting by name.
        let order = vec![4, 0, 2];

        // Anchor on the first row, click the last: everything drawn between them.
        assert_eq!(shift_range(&order, Some(4), 2), vec![4, 0, 2]);

        // Dragging upwards selects the same rows.
        assert_eq!(shift_range(&order, Some(2), 4), vec![4, 0, 2]);

        // A range that stops early must not sweep in item 1 or 3, which sit
        // between 0 and 2 numerically but are not on screen at all.
        assert_eq!(shift_range(&order, Some(0), 2), vec![0, 2]);
    }

    #[test]
    fn test_shift_range_without_anchor_selects_only_the_clicked_row() {
        let order = vec![4, 0, 2];
        assert_eq!(shift_range(&order, None, 0), vec![0]);
    }

    /// If the anchor was filtered out of the list since it was clicked, there is no
    /// meaningful range - selecting from a row the user can no longer see would
    /// silently pull in files that are not displayed.
    #[test]
    fn test_shift_range_ignores_anchor_that_is_no_longer_visible() {
        let order = vec![4, 0, 2];
        assert_eq!(shift_range(&order, Some(7), 0), vec![0]);
    }

    /// The search box scopes bulk actions. If this predicate ever matched more than
    /// what is drawn, Select All would hand hidden files to Lock.
    #[test]
    fn test_path_filter_is_case_insensitive_and_empty_matches_all() {
        let path = Path::new("/tmp/vault/Quarterly Report.pdf");

        assert!(path_matches_filter(path, ""), "An empty filter shows everything");
        assert!(path_matches_filter(path, "quarterly"), "Matching ignores case");
        assert!(path_matches_filter(path, "REPORT"), "Matching ignores case");
        assert!(path_matches_filter(path, "/tmp/vault"), "The directory is searchable too");
        assert!(!path_matches_filter(path, "invoice"), "Non-matching paths stay hidden");
    }

    fn dummy_item(path: &str) -> VaultItem {
        VaultItem {
            original_path: PathBuf::from(path),
            encrypted_path: None,
            is_locked: false,
            item_type: ItemType::File,
            is_folder_hidden: false,
            size: None,
        }
    }

    /// The virtualized list tells the scroll area a row count and then paints only a
    /// slice. If the flattened row list and the draw loop ever disagree, the
    /// scrollbar misreports and rows fall off the end - so pin the shape here.
    #[test]
    fn test_flatten_rows_emits_one_header_per_folder_plus_every_item() {
        let a = dummy_item("/vault/docs/a.pdf");
        let b = dummy_item("/vault/docs/b.pdf");
        let c = dummy_item("/vault/photos/c.jpg");

        let mut groups: BTreeMap<PathBuf, Vec<(usize, &VaultItem)>> = BTreeMap::new();
        groups.insert(PathBuf::from("/vault/docs"), vec![(0, &a), (1, &b)]);
        groups.insert(PathBuf::from("/vault/photos"), vec![(2, &c)]);

        let rows = flatten_rows(&groups);

        // 2 headers + 3 items
        assert_eq!(rows.len(), 5, "Row count drives the scroll area's virtual height");

        let headers = rows.iter().filter(|r| matches!(r, ListRow::Header { .. })).count();
        assert_eq!(headers, 2, "One header per folder group");

        // Items must appear under their own header, in group order.
        let item_indices: Vec<usize> = rows
            .iter()
            .filter_map(|r| match r {
                ListRow::Item { idx, .. } => Some(*idx),
                ListRow::Header { .. } => None,
            })
            .collect();
        assert_eq!(item_indices, vec![0, 1, 2]);

        assert!(
            matches!(rows[0], ListRow::Header { .. }),
            "A group starts with its header"
        );
        assert!(
            matches!(rows[3], ListRow::Header { .. }),
            "The second group's header follows the first group's items"
        );
    }

    /// The generator's meter divided the level by 100, so its bar sat at 2% for
    /// even a "Strong" verdict. All three meters share this now.
    #[test]
    fn test_strength_bar_fills_proportionally_and_maxes_out_at_strong() {
        assert!((strength_fill(0) - 1.0 / 3.0).abs() < f32::EPSILON, "Weak fills a third");
        assert!((strength_fill(1) - 2.0 / 3.0).abs() < f32::EPSILON, "Medium fills two thirds");
        assert!((strength_fill(2) - 1.0).abs() < f32::EPSILON, "Strong fills the bar");

        // Never overflow the track, whatever a future scorer returns.
        assert!((strength_fill(9) - 1.0).abs() < f32::EPSILON);

        assert!(strength_fill(0) < strength_fill(1) && strength_fill(1) < strength_fill(2));
    }

    /// Locked is the state the app exists to produce, so it must not be painted
    /// in the same tone as a failure. This pins the mapping that was inverted.
    #[test]
    fn test_password_strength_verdicts_map_to_sensible_tones() {
        let (_, weak_tone, weak) = assess_password_strength("abc");
        assert_eq!(weak, "Weak");
        assert_eq!(weak_tone, Tone::Bad);

        let (_, medium_tone, medium) = assess_password_strength("password1");
        assert_eq!(medium, "Medium");
        assert_eq!(medium_tone, Tone::Warn);

        let (level, strong_tone, strong) = assess_password_strength("Xk9#mQp2!vRt");
        assert_eq!(strong, "Strong");
        assert_eq!(strong_tone, Tone::Good);
        assert_eq!(level, 2, "Strong is the top level, so the bar fills");
    }

    /// Sequential and repeated runs are downgraded no matter how long the
    /// password is - worth pinning, since the meter is the only feedback the
    /// user gets while choosing a master password.
    #[test]
    fn test_obvious_patterns_are_rated_weak_despite_length() {
        assert_eq!(assess_password_strength("Abcdefgh123!").2, "Weak");
        assert_eq!(assess_password_strength("Paaassword12!").2, "Weak");
    }

    #[test]
    fn test_flatten_rows_of_an_empty_list_has_no_rows() {
        let groups: BTreeMap<PathBuf, Vec<(usize, &VaultItem)>> = BTreeMap::new();
        assert!(flatten_rows(&groups).is_empty());
    }

    #[test]
    fn test_format_file_size_reports_unmeasured_items_as_na() {
        assert_eq!(format_file_size(None), "N/A");
        assert_eq!(format_file_size(Some(0)), "0 B");
        assert_eq!(format_file_size(Some(512)), "512 B");
        assert_eq!(format_file_size(Some(2048)), "2.0 KB");
        assert_eq!(format_file_size(Some(5 * 1024 * 1024)), "5.0 MB");
        assert_eq!(format_file_size(Some(3 * 1024 * 1024 * 1024)), "3.00 GB");
    }

    #[test]
    fn test_original_path_rejects_non_vault_name() {
        let plain = PathBuf::from("/tmp/vault/notes.txt");
        assert!(
            MyVaultApp::original_path_for(&plain).is_none(),
            "A file without the vault suffix has no original name"
        );
    }
}
