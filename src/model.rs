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
}

