use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{ItemType, VaultItem};
use crate::platform;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub master_password_hash: Option<String>,
    pub salt: Option<String>,
    /// Data Encryption Key wrapped with the password-derived key (hex).
    /// Absent in vaults created before envelope encryption - those are migrated
    /// on the next successful login.
    #[serde(default)]
    pub wrapped_dek: Option<String>,
    pub vault_items: Vec<ConfigItem>,

    // Phase 2 & 3: Persistent UI preferences
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_true")]
    pub sort_ascending: bool,
    #[serde(default)]
    pub recent_files: Vec<String>,

    // Phase 3: Security settings
    #[serde(default = "default_session_timeout")]
    pub session_timeout_minutes: u64,
    #[serde(default = "default_true")]
    pub auto_lock_enabled: bool,
    #[serde(default = "default_password_reminder_days")]
    pub password_change_reminder_days: u64,
    #[serde(default)]
    pub password_last_changed: Option<u64>, // Unix timestamp
    #[serde(default)]
    pub reminder_dismissed_until: Option<u64>, // Unix timestamp
}

// Default value functions for serde
fn default_sort_by() -> String {
    "Name".to_string()
}

fn default_true() -> bool {
    true
}

fn default_session_timeout() -> u64 {
    15 // 15 minutes default
}

fn default_password_reminder_days() -> u64 {
    90 // 90 days default
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigItem {
    pub original_path: String,
    pub encrypted_path: Option<String>,
    pub is_locked: bool,
    pub item_type: ItemType,
    #[serde(default)]
    pub is_folder_hidden: bool,
}

impl From<&VaultItem> for ConfigItem {
    fn from(v: &VaultItem) -> Self {
        Self {
            original_path: v.original_path.to_string_lossy().into_owned(),
            encrypted_path: v
                .encrypted_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            is_locked: v.is_locked,
            item_type: v.item_type,
            is_folder_hidden: v.is_folder_hidden,
        }
    }
}

impl From<&ConfigItem> for VaultItem {
    fn from(c: &ConfigItem) -> Self {
        Self {
            original_path: PathBuf::from(&c.original_path),
            encrypted_path: c.encrypted_path.as_ref().map(PathBuf::from),
            is_locked: c.is_locked,
            item_type: c.item_type,
            is_folder_hidden: c.is_folder_hidden,
        }
    }
}

pub fn config_dir() -> PathBuf {
    // Use the cross-platform platform module
    // Falls back to old behavior if platform::config_dir() fails
    platform::config_dir()
        .unwrap_or_else(|_| {
            // Fallback to previous behavior
            #[cfg(target_os = "windows")]
            {
                let base = std::env::var_os("APPDATA").unwrap_or_else(|| "".into());
                let mut p = PathBuf::from(base);
                p.push("MyVault");
                p
            }
            #[cfg(not(target_os = "windows"))]
            {
                let home = std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
                    .unwrap_or_else(|| PathBuf::from("."));
                let p = if home.ends_with(".config") {
                    home
                } else {
                    let mut h = home;
                    h.push(".config");
                    h
                };
                p.join("myvault")
            }
        })
}

pub fn config_path() -> PathBuf {
    config_dir().join("vault_config.json")
}

pub fn load_config() -> io::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = fs::read(&path)?;
    match serde_json::from_slice(&data) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            // Never silently fall back to a default config: doing so would drop the
            // password hash, salt and wrapped DEK, and the next save would overwrite
            // the only copy of the key material. Preserve the file and report instead.
            let backup = path.with_extension(format!(
                "json.corrupt-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ));
            let _ = fs::copy(&path, &backup);
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "vault_config.json could not be read ({}). A copy was kept at {}",
                    e,
                    backup.display()
                ),
            ))
        }
    }
}

pub fn save_config(cfg: &Config) -> io::Result<()> {
    // Bail out rather than writing "{}" over a valid config: that would destroy the
    // wrapped DEK and make every encrypted file unrecoverable.
    let json = serde_json::to_vec_pretty(cfg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(&path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.flush()?;
        // fsync the temp file before renaming, so a crash can never leave a
        // config that exists but has no key material in it.
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn tmp_path(target: &Path) -> PathBuf {
    let mut p = target.to_path_buf();
    if let Some(ext) = p.extension() {
        let mut e = ext.to_os_string();
        e.push(".tmp");
        p.set_extension(e);
    } else {
        p.set_extension("tmp");
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config written before envelope encryption has no `wrapped_dek` field.
    /// It must still load, with the field absent so login can migrate it.
    #[test]
    fn test_legacy_config_without_wrapped_dek_loads() {
        let legacy = r#"{
            "master_password_hash": "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
            "salt": "c2FsdHNhbHRzYWx0",
            "vault_items": []
        }"#;

        let cfg: Config = serde_json::from_str(legacy).expect("legacy config should deserialize");
        assert!(cfg.wrapped_dek.is_none(), "Legacy config has no wrapped DEK");
        assert!(cfg.master_password_hash.is_some());
        assert_eq!(cfg.salt.as_deref(), Some("c2FsdHNhbHRzYWx0"));
    }

    #[test]
    fn test_wrapped_dek_survives_serialization() {
        let cfg = Config {
            wrapped_dek: Some("deadbeef".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wrapped_dek.as_deref(), Some("deadbeef"));
    }
}

