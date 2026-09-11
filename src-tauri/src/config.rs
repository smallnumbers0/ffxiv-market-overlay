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
/// Favorites are deliberate, not automatic, so the cap only exists to stop a
/// hand-edited config from growing without bound. Nobody curates a hundred.
const MAX_FAVORITE_ITEMS: usize = 100;

/// Id of the board every config starts with.
pub const FIRST_BOARD: &str = "board-1";
/// Prefix for generated board ids.
const BOARD_PREFIX: &str = "board-";

/// Ceiling on open boards. Every board is a column in the same window, so
/// this is bounded by pixels rather than taste: past four, columns are too
/// narrow to read even with the strip scrolling.
pub const MAX_BOARDS: usize = 4;

/// One market board the user is watching.
///
/// Every board is shown at once, as its own column beside the others. There is
/// no "current" board: you pick an item once and read every board's price for
/// it side by side, which is the whole reason for having more than one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardConfig {
    pub id: String,
    /// World or data center name passed straight to Universalis. Never a
    /// region - see `MarketScopes::widen`. `None` only until first launch
    /// resolves one from the system clock.
    pub scope: Option<String>,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self::new(FIRST_BOARD)
    }
}

impl BoardConfig {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            scope: None,
        }
    }

    /// The scope to query, or an error explaining that setup isn't done.
    pub fn require_scope(&self) -> AppResult<&str> {
        self.scope
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::Invalid("pick your home world or data center in settings first".into())
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// Boards being compared, left to right. Never empty - see `migrate`.
    ///
    /// `tabs` is the name an earlier build wrote, kept as an alias so those
    /// configs keep their boards.
    #[serde(alias = "tabs")]
    pub boards: Vec<BoardConfig>,
    /// Accelerator string for the show/hide hotkey.
    pub hotkey: String,
    /// Last overlay position/size, restored on show.
    pub window: Option<WindowState>,
    /// Most recently viewed items, newest first. Shared across boards - these
    /// are items the user looked at, not a property of any one board.
    pub recent_item_ids: Vec<u32>,
    /// Hearted items, newest first. Unlike recents these are chosen, so the
    /// list only changes when the user says so.
    pub favorite_item_ids: Vec<u32>,

    // --- Pre-comparison fields -------------------------------------------
    // Read so upgrading keeps the user's world, folded into `boards` by
    // `migrate`, and never written back out.
    #[serde(rename = "marketScope", skip_serializing)]
    legacy_scope: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            boards: vec![BoardConfig::new(FIRST_BOARD)],
            hotkey: DEFAULT_HOTKEY.to_string(),
            window: None,
            recent_item_ids: Vec::new(),
            favorite_item_ids: Vec::new(),
            legacy_scope: None,
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
    pub fn board(&self, id: &str) -> Option<&BoardConfig> {
        self.boards.iter().find(|board| board.id == id)
    }

    /// The board a request names. An id with no board behind it means the UI
    /// is working from a stale copy of the board list.
    pub fn require_board(&self, id: &str) -> AppResult<&BoardConfig> {
        self.board(id)
            .ok_or_else(|| AppError::Invalid(format!("that board is gone: '{id}'")))
    }

    /// Fold any pre-comparison fields into the board list and guarantee the
    /// invariant every other method leans on: there is always a board.
    fn migrate(&mut self) {
        if self.boards.is_empty() {
            self.boards.push(BoardConfig::new(FIRST_BOARD));
        }
        if let Some(scope) = self.legacy_scope.take() {
            // Only the first board, and only if it has nothing of its own: a
            // config already carrying boards was written by this version.
            let first = &mut self.boards[0];
            if first.scope.is_none() {
                first.scope = Some(scope);
            }
        }
        self.legacy_scope = None;

        // A hand-edited config must not be able to open a hundred columns.
        self.boards.truncate(MAX_BOARDS);
    }

    /// An id no open board is using, for the next one.
    fn free_board_id(&self) -> String {
        (1..)
            .map(|n| format!("{BOARD_PREFIX}{n}"))
            .find(|id| self.board(id).is_none())
            .expect("an unbounded search always finds a free id")
    }

    fn push_recent(&mut self, item_id: u32) {
        self.recent_item_ids.retain(|id| *id != item_id);
        self.recent_item_ids.insert(0, item_id);
        self.recent_item_ids.truncate(MAX_RECENT_ITEMS);
    }

    pub fn is_favorite(&self, item_id: u32) -> bool {
        self.favorite_item_ids.contains(&item_id)
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
        let mut config = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(&raw).ok())
            .unwrap_or_default();
        config.migrate();
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

    /// Mutate one board and persist. Errors rather than silently doing nothing
    /// when the board is gone, so a failed world change is visible in the UI.
    fn update_board<F>(&self, id: &str, mutate: F) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut BoardConfig),
    {
        self.get().require_board(id)?;
        self.update(|config| {
            if let Some(board) = config.boards.iter_mut().find(|board| board.id == id) {
                mutate(board);
            }
        })
    }

    pub fn set_board_scope(&self, id: &str, scope: &str) -> AppResult<AppConfig> {
        let scope = validate_scope(scope)?;
        self.update_board(id, |board| {
            board.scope = Some(scope);
        })
    }

    /// Set a first-launch scope, but never overwrite a choice already made.
    ///
    /// Returning the config unchanged when a scope already exists is what
    /// makes this safe to call on every startup.
    pub fn set_guessed_board_scope(&self, id: &str, scope: &str) -> AppResult<AppConfig> {
        let scope = validate_scope(scope)?;
        if self.get().require_board(id)?.scope.is_some() {
            return Ok(self.get());
        }
        self.update_board(id, |board| {
            board.scope = Some(scope);
        })
    }

    /// Add a board as a new right-hand column.
    pub fn add_board(&self, scope: Option<String>) -> AppResult<AppConfig> {
        let config = self.get();
        if config.boards.len() >= MAX_BOARDS {
            return Err(AppError::Invalid(format!(
                "{MAX_BOARDS} boards is the maximum - close one first"
            )));
        }
        let board = BoardConfig {
            id: config.free_board_id(),
            scope,
        };
        self.update(|config| config.boards.push(board))
    }

    /// Close a board's column. The last one stays: an overlay with no board
    /// to show is just an empty window.
    pub fn remove_board(&self, id: &str) -> AppResult<AppConfig> {
        let config = self.get();
        config.require_board(id)?;
        if config.boards.len() == 1 {
            return Err(AppError::Invalid(
                "that's the only board - press Esc or the hotkey to hide the overlay".into(),
            ));
        }
        self.update(|config| config.boards.retain(|board| board.id != id))
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

    /// Heart an item, or un-heart it if it is already hearted. Returns the
    /// config so the caller can report the new state without a second read.
    ///
    /// Refusing at the cap rather than dropping the oldest: a favorite that
    /// silently disappeared would be worse than being told the list is full.
    pub fn toggle_favorite(&self, item_id: u32) -> AppResult<AppConfig> {
        let config = self.get();
        if config.is_favorite(item_id) {
            return self.update(|config| config.favorite_item_ids.retain(|id| *id != item_id));
        }
        if config.favorite_item_ids.len() >= MAX_FAVORITE_ITEMS {
            return Err(AppError::Invalid(format!(
                "{MAX_FAVORITE_ITEMS} favorites is the maximum - remove one first"
            )));
        }
        self.update(|config| config.favorite_item_ids.insert(0, item_id))
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

/// Worlds and data centers both arrive here as free text from the
/// picker; the only rule is that a board name has to be something.
fn validate_scope(scope: &str) -> AppResult<String> {
    let scope = scope.trim();
    if scope.is_empty() {
        return Err(AppError::Invalid(
            "world or data center cannot be empty".into(),
        ));
    }
    Ok(scope.to_string())
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

    fn scope_of(config: &AppConfig, id: &str) -> Option<String> {
        config.board(id).and_then(|board| board.scope.clone())
    }

    fn ids(config: &AppConfig) -> Vec<String> {
        config.boards.iter().map(|b| b.id.clone()).collect()
    }

    /// The id `add_board` just handed out - always the new last column.
    fn newest(config: &AppConfig) -> String {
        config.boards.last().unwrap().id.clone()
    }

    #[test]
    fn defaults_when_no_file_exists() {
        let (_dir, store) = store();
        let config = store.get();
        assert_eq!(config.boards.len(), 1, "there is always a board");
        assert_eq!(scope_of(&config, FIRST_BOARD), None);
        assert_eq!(config.hotkey, DEFAULT_HOTKEY);
        assert!(config.recent_item_ids.is_empty());
    }

    #[test]
    fn settings_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let store = ConfigStore::load(&path);
        store.set_board_scope(FIRST_BOARD, "Cactuar").unwrap();
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
        assert_eq!(scope_of(&reloaded, FIRST_BOARD).as_deref(), Some("Cactuar"));
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
    fn boards_reopen_in_order_on_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let store = ConfigStore::load(&path);
        store.set_board_scope(FIRST_BOARD, "Cactuar").unwrap();
        let second = newest(&store.add_board(Some("Aether".into())).unwrap());

        let reloaded = ConfigStore::load(&path).get();
        assert_eq!(
            ids(&reloaded),
            vec![FIRST_BOARD.to_string(), second.clone()]
        );
        assert_eq!(scope_of(&reloaded, FIRST_BOARD).as_deref(), Some("Cactuar"));
        assert_eq!(scope_of(&reloaded, &second).as_deref(), Some("Aether"));
    }

    #[test]
    fn boards_hold_scopes_independently() {
        let (_dir, store) = store();
        store.set_board_scope(FIRST_BOARD, "Cactuar").unwrap();
        let second = newest(&store.add_board(Some("Aether".into())).unwrap());

        // The whole point of a second column: moving one leaves the other.
        let config = store.set_board_scope(&second, "Primal").unwrap();
        assert_eq!(scope_of(&config, FIRST_BOARD).as_deref(), Some("Cactuar"));
        assert_eq!(scope_of(&config, &second).as_deref(), Some("Primal"));
    }

    #[test]
    fn a_new_board_becomes_the_right_hand_column() {
        let (_dir, store) = store();
        let second = newest(&store.add_board(None).unwrap());
        let third = newest(&store.add_board(None).unwrap());
        assert_eq!(
            ids(&store.get()),
            vec![FIRST_BOARD.to_string(), second, third]
        );
    }

    #[test]
    fn ids_are_reused_once_a_board_closes() {
        let (_dir, store) = store();
        let second = newest(&store.add_board(None).unwrap());
        let third = newest(&store.add_board(None).unwrap());
        assert_ne!(second, third);

        store.remove_board(&second).unwrap();
        let fourth = newest(&store.add_board(None).unwrap());
        assert_eq!(fourth, second, "the freed id comes back");
        assert_eq!(store.get().boards.len(), 3);
    }

    #[test]
    fn the_last_board_cannot_be_closed() {
        let (_dir, store) = store();
        assert_eq!(
            store.remove_board(FIRST_BOARD).unwrap_err().kind(),
            "invalid"
        );
        assert_eq!(store.get().boards.len(), 1);
    }

    #[test]
    fn boards_are_capped() {
        let (_dir, store) = store();
        for _ in 1..MAX_BOARDS {
            store.add_board(None).unwrap();
        }
        assert_eq!(store.get().boards.len(), MAX_BOARDS);
        assert_eq!(store.add_board(None).unwrap_err().kind(), "invalid");
    }

    #[test]
    fn changes_to_a_missing_board_are_an_error() {
        let (_dir, store) = store();
        assert_eq!(
            store
                .set_board_scope("board-9", "Cactuar")
                .unwrap_err()
                .kind(),
            "invalid"
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
        std::fs::write(
            &path,
            r#"{"boards":[{"id":"board-1","scope":"Aether"}],"futureField":42}"#,
        )
        .unwrap();
        let config = ConfigStore::load(&path).get();
        assert_eq!(scope_of(&config, FIRST_BOARD).as_deref(), Some("Aether"));
        assert_eq!(
            config.hotkey, DEFAULT_HOTKEY,
            "missing field takes the default"
        );
    }

    /// Shipped builds wrote a per-board `scopeIsGuess`, which drove a
    /// first-launch banner this version no longer has. Those configs must
    /// still load every board - dropping one would lose a user's column.
    #[test]
    fn the_retired_guess_flag_is_ignored_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"boards":[{"id":"board-1","scope":"Marilith","scopeIsGuess":false},
                          {"id":"board-2","scope":"Aether","scopeIsGuess":true}],
                "hotkey":"CmdOrControl+Shift+M"}"#,
        )
        .unwrap();

        let store = ConfigStore::load(&path);
        let config = store.get();
        assert_eq!(config.boards.len(), 2);
        assert_eq!(scope_of(&config, "board-1").as_deref(), Some("Marilith"));
        assert_eq!(scope_of(&config, "board-2").as_deref(), Some("Aether"));

        // And the key does not survive the next write.
        store.set_hotkey("Alt+N").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("scopeIsGuess"));
    }

    /// The build that shipped tabs wrote this key. Those users should keep
    /// their boards rather than silently dropping back to one.
    #[test]
    fn boards_saved_under_the_old_tabs_key_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"tabs":[{"id":"tab-1","scope":"Cactuar"},{"id":"tab-2","scope":"Aether"}],
                "activeTab":"tab-2"}"#,
        )
        .unwrap();
        let config = ConfigStore::load(&path).get();
        assert_eq!(config.boards.len(), 2);
        assert_eq!(scope_of(&config, "tab-1").as_deref(), Some("Cactuar"));
        assert_eq!(scope_of(&config, "tab-2").as_deref(), Some("Aether"));
    }

    /// Upgrading from the single-board version must not dump the user back on
    /// the setup screen.
    #[test]
    fn a_pre_comparison_config_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"marketScope":"Cactuar","marketScopeIsGuess":false,
                "hotkey":"Alt+M","recentItemIds":[5,6],
                "window":{"x":1,"y":2,"width":300,"height":400}}"#,
        )
        .unwrap();

        let store = ConfigStore::load(&path);
        let config = store.get();
        let board = config.board(FIRST_BOARD).unwrap();
        assert_eq!(board.scope.as_deref(), Some("Cactuar"));
        assert_eq!(config.window.unwrap().width, 300);
        assert_eq!(config.hotkey, "Alt+M");
        assert_eq!(config.recent_item_ids, vec![5, 6]);

        // And the old keys are gone from disk once anything is written.
        store.set_hotkey("Alt+N").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("marketScope"),
            "legacy keys are not rewritten"
        );
        assert!(raw.contains("boards"));
    }

    #[test]
    fn a_first_launch_scope_never_overwrites_a_real_choice() {
        let (_dir, store) = store();

        let config = store
            .set_guessed_board_scope(FIRST_BOARD, "Aether")
            .unwrap();
        assert_eq!(scope_of(&config, FIRST_BOARD).as_deref(), Some("Aether"));

        // The user picks their own world.
        let config = store.set_board_scope(FIRST_BOARD, "Cactuar").unwrap();
        assert_eq!(scope_of(&config, FIRST_BOARD).as_deref(), Some("Cactuar"));

        // A later startup must not clobber it.
        let config = store.set_guessed_board_scope(FIRST_BOARD, "Chaos").unwrap();
        assert_eq!(scope_of(&config, FIRST_BOARD).as_deref(), Some("Cactuar"));
    }

    #[test]
    fn blank_scope_and_hotkey_are_rejected() {
        let (_dir, store) = store();
        assert_eq!(
            store
                .set_board_scope(FIRST_BOARD, "   ")
                .unwrap_err()
                .kind(),
            "invalid"
        );
        assert_eq!(store.set_hotkey("").unwrap_err().kind(), "invalid");
    }

    #[test]
    fn require_scope_explains_what_to_do() {
        let err = BoardConfig::new(FIRST_BOARD).require_scope().unwrap_err();
        assert!(err.to_string().contains("settings"));
    }

    #[test]
    fn favorites_toggle_and_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = ConfigStore::load(&path);

        store.toggle_favorite(4745).unwrap();
        let config = store.toggle_favorite(5057).unwrap();
        assert_eq!(config.favorite_item_ids, vec![5057, 4745], "newest first");
        assert!(config.is_favorite(4745));

        // The same item again un-hearts it rather than duplicating it.
        let config = store.toggle_favorite(4745).unwrap();
        assert_eq!(config.favorite_item_ids, vec![5057]);
        assert!(!config.is_favorite(4745));

        assert_eq!(ConfigStore::load(&path).get().favorite_item_ids, vec![5057]);
    }

    /// Unlike recents, a favorite the user chose must never be silently
    /// dropped to make room.
    #[test]
    fn favorites_are_capped_without_losing_any() {
        let (_dir, store) = store();
        for id in 1..=(MAX_FAVORITE_ITEMS as u32) {
            store.toggle_favorite(id).unwrap();
        }
        assert_eq!(store.get().favorite_item_ids.len(), MAX_FAVORITE_ITEMS);

        let err = store.toggle_favorite(9999).unwrap_err();
        assert_eq!(err.kind(), "invalid");
        assert_eq!(
            store.get().favorite_item_ids.len(),
            MAX_FAVORITE_ITEMS,
            "nothing was evicted"
        );

        // Un-hearting still works at the cap.
        store.toggle_favorite(1).unwrap();
        assert!(store.toggle_favorite(9999).is_ok());
    }

    #[test]
    fn favorites_and_recents_are_independent() {
        let (_dir, store) = store();
        store.push_recent(4745).unwrap();
        store.toggle_favorite(4745).unwrap();

        store.clear_recents().unwrap();
        let config = store.get();
        assert!(config.recent_item_ids.is_empty());
        assert!(
            config.is_favorite(4745),
            "clearing recents must not touch a hearted item"
        );
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
