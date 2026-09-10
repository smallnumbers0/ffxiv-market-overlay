//! User settings, persisted as JSON in the OS app-config directory.
//!
//! Hand-rolled over `tauri-plugin-store` because the Rust side needs these
//! values before any webview exists - the global hotkey is registered at
//! startup, from `run()`.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const DEFAULT_HOTKEY: &str = "CmdOrControl+Shift+M";
const MAX_RECENT_ITEMS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// World, data center, or region name passed straight to Universalis.
    /// `None` only until first launch guesses a region from the system clock.
    pub market_scope: Option<String>,
    /// True while `market_scope` is a first-launch guess the user has never
    /// confirmed. Drives the "these are region-wide prices, pick your world"
    /// nudge, and is cleared the moment they choose one themselves.
    pub market_scope_is_guess: bool,
    /// Accelerator string for the show/hide hotkey.
    pub hotkey: String,
    /// Last overlay position/size, restored on show.
    pub window: Option<WindowState>,
    /// Most recently viewed items, newest first.
    pub recent_item_ids: Vec<u32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            market_scope: None,
            market_scope_is_guess: false,
            hotkey: DEFAULT_HOTKEY.to_string(),
            window: None,
            recent_item_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl AppConfig {
    /// The scope to query, or an error explaining that setup isn't done.
    pub fn require_scope(&self) -> AppResult<&str> {
        self.market_scope
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::Invalid("pick your home world or data center in settings first".into())
            })
    }

    fn push_recent(&mut self, item_id: u32) {
        self.recent_item_ids.retain(|id| *id != item_id);
        self.recent_item_ids.insert(0, item_id);
        self.recent_item_ids.truncate(MAX_RECENT_ITEMS);
    }
}

/// Reads at startup, writes through on every change. Cheap enough to save
/// synchronously: the file is a few hundred bytes and changes are user-paced.
pub struct ConfigStore {
    path: PathBuf,
    config: RwLock<AppConfig>,
}

impl ConfigStore {
    /// Load from `path`, falling back to defaults when the file is absent or
    /// unreadable. A corrupt config must never stop the app from starting.
    pub fn load(path: &Path) -> Self {
        let config = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(&raw).ok())
            .unwrap_or_default();
        Self {
            path: path.to_path_buf(),
            config: RwLock::new(config),
        }
    }

    pub fn get(&self) -> AppConfig {
        self.config
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }

    /// Mutate and persist in one step. The closure sees the live config so
    /// callers can't accidentally write back a stale copy.
    pub fn update<F>(&self, mutate: F) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig),
    {
        let updated = {
            let mut config = self
                .config
                .write()
                .map_err(|_| AppError::Config("settings lock poisoned".into()))?;
            mutate(&mut config);
            config.clone()
        };
        self.save(&updated)?;
        Ok(updated)
    }

    pub fn set_market_scope(&self, scope: &str) -> AppResult<AppConfig> {
        let scope = scope.trim();
        if scope.is_empty() {
            return Err(AppError::Invalid(
                "world or data center cannot be empty".into(),
            ));
        }
        let scope = scope.to_string();
        self.update(|config| {
            config.market_scope = Some(scope);
            config.market_scope_is_guess = false;
        })
    }

    /// Set a first-launch guess, but never overwrite a real choice.
    ///
    /// Returning the config unchanged when a scope already exists is what
    /// makes this safe to call on every startup.
    pub fn set_guessed_market_scope(&self, scope: &str) -> AppResult<AppConfig> {
        let scope = scope.trim();
        if scope.is_empty() {
            return Err(AppError::Invalid(
                "world or data center cannot be empty".into(),
            ));
        }
        if self.get().market_scope.is_some() {
            return Ok(self.get());
        }
        let scope = scope.to_string();
        self.update(|config| {
            config.market_scope = Some(scope);
            config.market_scope_is_guess = true;
        })
    }

    pub fn set_hotkey(&self, hotkey: &str) -> AppResult<AppConfig> {
        let hotkey = hotkey.trim();
        if hotkey.is_empty() {
            return Err(AppError::Invalid("hotkey cannot be empty".into()));
        }
        let hotkey = hotkey.to_string();
        self.update(|config| config.hotkey = hotkey)
    }

    pub fn set_window(&self, window: WindowState) -> AppResult<AppConfig> {
        self.update(|config| config.window = Some(window))
    }

    pub fn push_recent(&self, item_id: u32) -> AppResult<AppConfig> {
        self.update(|config| config.push_recent(item_id))
    }

    pub fn clear_recents(&self) -> AppResult<AppConfig> {
        self.update(|config| config.recent_item_ids.clear())
    }

    fn save(&self, config: &AppConfig) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Config(format!("{}: {e}", parent.display())))?;
        }
        let json =
            serde_json::to_string_pretty(config).map_err(|e| AppError::Config(e.to_string()))?;
        // Write-then-rename so a crash mid-write can't leave a truncated file
        // that would silently reset the user's settings on next launch.
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, json)
            .map_err(|e| AppError::Config(format!("{}: {e}", temp.display())))?;
        std::fs::rename(&temp, &self.path)
            .map_err(|e| AppError::Config(format!("{}: {e}", self.path.display())))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ConfigStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.json");
        let store = ConfigStore::load(&path);
        (dir, store)
    }

    #[test]
    fn defaults_when_no_file_exists() {
        let (_dir, store) = store();
        let config = store.get();
        assert_eq!(config.market_scope, None);
        assert_eq!(config.hotkey, DEFAULT_HOTKEY);
        assert!(config.recent_item_ids.is_empty());
    }

    #[test]
    fn settings_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let store = ConfigStore::load(&path);
        store.set_market_scope("Cactuar").unwrap();
        store.set_hotkey("Alt+M").unwrap();
        store
            .set_window(WindowState {
                x: 10,
                y: 20,
                width: 500,
                height: 600,
            })
            .unwrap();

        let reloaded = ConfigStore::load(&path).get();
        assert_eq!(reloaded.market_scope.as_deref(), Some("Cactuar"));
        assert_eq!(reloaded.hotkey, "Alt+M");
        assert_eq!(
            reloaded.window,
            Some(WindowState {
                x: 10,
                y: 20,
                width: 500,
                height: 600
            })
        );
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(ConfigStore::load(&path).get(), AppConfig::default());
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"marketScope":"Aether","futureField":42}"#).unwrap();
        let config = ConfigStore::load(&path).get();
        assert_eq!(config.market_scope.as_deref(), Some("Aether"));
        assert_eq!(
            config.hotkey, DEFAULT_HOTKEY,
            "missing field takes the default"
        );
    }

    #[test]
    fn a_guessed_scope_never_overwrites_a_real_choice() {
        let (_dir, store) = store();

        let config = store.set_guessed_market_scope("North-America").unwrap();
        assert_eq!(config.market_scope.as_deref(), Some("North-America"));
        assert!(config.market_scope_is_guess);

        // The user picks their own world - the guess flag goes away.
        let config = store.set_market_scope("Cactuar").unwrap();
        assert!(!config.market_scope_is_guess);

        // A later startup guess must not clobber it.
        let config = store.set_guessed_market_scope("Europe").unwrap();
        assert_eq!(config.market_scope.as_deref(), Some("Cactuar"));
        assert!(!config.market_scope_is_guess);
    }

    #[test]
    fn blank_scope_and_hotkey_are_rejected() {
        let (_dir, store) = store();
        assert_eq!(store.set_market_scope("   ").unwrap_err().kind(), "invalid");
        assert_eq!(store.set_hotkey("").unwrap_err().kind(), "invalid");
    }

    #[test]
    fn require_scope_explains_what_to_do() {
        let config = AppConfig::default();
        let err = config.require_scope().unwrap_err();
        assert!(err.to_string().contains("settings"));
    }

    #[test]
    fn recents_are_deduped_newest_first_and_capped() {
        let (_dir, store) = store();
        for id in 1..=(MAX_RECENT_ITEMS as u32 + 3) {
            store.push_recent(id).unwrap();
        }
        store.push_recent(2).unwrap();

        let recents = store.get().recent_item_ids;
        assert_eq!(recents.len(), MAX_RECENT_ITEMS);
        assert_eq!(recents[0], 2, "re-viewing moves an item to the front");
        assert_eq!(recents.iter().filter(|id| **id == 2).count(), 1);

        store.clear_recents().unwrap();
        assert!(store.get().recent_item_ids.is_empty());
    }
}
