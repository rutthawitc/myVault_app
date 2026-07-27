use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ItemType {
    File,
    Folder,
}

#[derive(Debug, Clone)]
pub struct VaultItem {
    pub original_path: PathBuf,
    pub encrypted_path: Option<PathBuf>,
    pub is_locked: bool,
    pub item_type: ItemType,
    pub is_folder_hidden: bool,
    /// Last known size in bytes, cached so the UI never calls `metadata()` while
    /// painting. The file list is repainted many times per second; stat-ing every
    /// row every frame turned into thousands of syscalls per second.
    ///
    /// `None` means "never successfully measured". A locked item keeps the size it
    /// had before encryption, because its original path no longer exists.
    pub size: Option<u64>,
}

