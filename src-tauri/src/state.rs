//! Shared application state, built once at startup and handed to every command.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard};

use crate::cache::PriceCache;
use crate::commands::{BoardView, Settings};
use crate::config::ConfigStore;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::search::SearchIndex;
use crate::universalis::{MarketScopes, UniversalisClient};

pub struct AppState {
    pub config: ConfigStore,
    pub cache: PriceCache,
    pub universalis: UniversalisClient,
    /// The writable app-data copy of `items.db` - the bundled resource is
    /// read-only and may live inside a signed app bundle.
    pub catalog_path: PathBuf,
    search: RwLock<SearchIndex>,
    /// Why the global hotkey isn't active, or `None` when it is. Surfaced in
    /// the settings panel: a hotkey silently failing to register is the one
    /// failure that leaves the user with no way to open the overlay, so it
    /// must never be a log line nobody reads.
    hotkey_error: RwLock<Option<String>>,
    /// Shared with the `SyncGuard` handed out by `begin_sync`, so the flag is
    /// cleared however the sync ends.
    syncing: Arc<AtomicBool>,
    /// The world/DC list, fetched at most once per session. Both the settings
    /// picker and every new pane need it, and it only changes when Square Enix
    /// adds a world.
    scopes: tokio::sync::OnceCell<MarketScopes>,
}

impl AppState {
    /// Build state from the two paths the app owns. A missing or unreadable
    /// catalog is not fatal: the app starts with an empty index and the UI
    /// prompts the user to sync.
    pub fn new(config_path: &Path, catalog_path: &Path) -> AppResult<Self> {
        let items = load_items(catalog_path);
        Ok(Self {
            config: ConfigStore::load(config_path),
            cache: PriceCache::default(),
            universalis: UniversalisClient::new()?,
            catalog_path: catalog_path.to_path_buf(),
            search: RwLock::new(SearchIndex::new(items)),
            hotkey_error: RwLock::new(None),
            syncing: Arc::new(AtomicBool::new(false)),
            scopes: tokio::sync::OnceCell::new(),
        })
    }

    pub fn search_index(&self) -> AppResult<RwLockReadGuard<'_, SearchIndex>> {
        self.search
            .read()
            .map_err(|_| AppError::Internal("search index lock poisoned".into()))
    }

    /// Swap in a freshly synced catalog without restarting the app.
    pub fn reload_search_index(&self) -> AppResult<usize> {
        let conn = db::open_read_only(&self.catalog_path)?;
        let items = db::load_marketable_items(&conn)?;
        let count = items.len();
        *self
            .search
            .write()
            .map_err(|_| AppError::Internal("search index lock poisoned".into()))? =
            SearchIndex::new(items);
        Ok(count)
    }

    pub fn set_hotkey_error(&self, error: Option<String>) {
        if let Ok(mut slot) = self.hotkey_error.write() {
            *slot = error;
        }
    }

    pub fn hotkey_error(&self) -> Option<String> {
        self.hotkey_error.read().ok().and_then(|slot| slot.clone())
    }

    pub fn catalog_ready(&self) -> bool {
        self.search_index().map(|i| !i.is_empty()).unwrap_or(false)
    }

    /// Everything the UI needs: every board and the state they share.
    pub fn settings(&self) -> AppResult<Settings> {
        // Stats come from the file rather than the in-memory index so the
        // panel can still report a catalog the index failed to load.
        let catalog = db::open_read_only(&self.catalog_path)
            .and_then(|conn| db::catalog_info(&conn))
            .unwrap_or_default();
        let config = self.config.get();

        Ok(Settings {
            can_add_board: config.boards.len() < crate::config::MAX_BOARDS,
            boards: config
                .boards
                .into_iter()
                .map(|board| BoardView {
                    id: board.id,
                    scope: board.scope,
                })
                .collect(),
            hotkey: config.hotkey,
            recent_item_ids: config.recent_item_ids,
            favorite_item_ids: config.favorite_item_ids,
            catalog,
            catalog_ready: self.catalog_ready(),
            hotkey_error: self.hotkey_error(),
        })
    }

    /// The world/DC list, fetched on first use and kept for the session.
    ///
    /// A failed fetch is not memoised, so opening the settings panel again
    /// after the network comes back retries instead of serving the error.
    pub async fn market_scopes(&self) -> AppResult<&MarketScopes> {
        self.scopes
            .get_or_try_init(|| async {
                let (worlds, data_centers) = tokio::try_join!(
                    self.universalis.fetch_worlds(),
                    self.universalis.fetch_data_centers()
                )?;
                Ok(MarketScopes {
                    worlds,
                    data_centers,
                })
            })
            .await
    }

    /// Claim the right to run a catalog sync. Returns an error if one is
    /// already running - two concurrent syncs would fight over the same file.
    pub fn begin_sync(&self) -> AppResult<SyncGuard> {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return Err(AppError::Invalid(
                "a catalog sync is already running".into(),
            ));
        }
        Ok(SyncGuard {
            flag: Arc::clone(&self.syncing),
        })
    }
}

/// Clears the in-progress flag when a sync ends, however it ends.
#[derive(Debug)]
pub struct SyncGuard {
    flag: Arc<AtomicBool>,
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

fn load_items(catalog_path: &Path) -> Vec<db::Item> {
    db::open_read_only(catalog_path)
        .and_then(|conn| db::load_marketable_items(&conn))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ItemRow;

    fn row(id: u32, name: &str) -> ItemRow {
        ItemRow {
            item_id: id,
            name: name.to_string(),
            icon_path: None,
            level_item: None,
            category_name: None,
            marketable: true,
        }
    }

    /// State over a temp dir, with `items` already in the catalog.
    fn state(dir: &Path, items: &[ItemRow]) -> AppState {
        let catalog = dir.join("items.db");
        let mut conn = db::open_read_write(&catalog).unwrap();
        db::replace_items(&mut conn, items).unwrap();
        drop(conn);
        AppState::new(&dir.join("config.json"), &catalog).unwrap()
    }

    #[test]
    fn starts_with_the_catalog_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path(), &[row(1, "Hi-Potion"), row(2, "Cobalt Ingot")]);

        assert!(state.catalog_ready());
        assert_eq!(state.search_index().unwrap().len(), 2);
    }

    #[test]
    fn a_missing_catalog_is_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let state =
            AppState::new(&dir.path().join("config.json"), &dir.path().join("gone.db")).unwrap();

        assert!(!state.catalog_ready(), "the UI prompts for a sync instead");
        assert!(state.search_index().unwrap().is_empty());
        assert!(state.settings().is_ok(), "settings still render");
    }

    #[test]
    fn reload_picks_up_a_freshly_synced_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path(), &[row(1, "Hi-Potion")]);
        assert_eq!(state.search_index().unwrap().len(), 1);

        let mut conn = db::open_read_write(&dir.path().join("items.db")).unwrap();
        db::replace_items(&mut conn, &[row(1, "Hi-Potion"), row(2, "Potion")]).unwrap();
        drop(conn);

        assert_eq!(state.reload_search_index().unwrap(), 2);
        assert_eq!(state.search_index().unwrap().len(), 2);
    }

    #[test]
    fn only_one_sync_runs_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path(), &[row(1, "Hi-Potion")]);

        let guard = state.begin_sync().unwrap();
        assert_eq!(
            state.begin_sync().unwrap_err().kind(),
            "invalid",
            "a second concurrent sync is refused"
        );

        drop(guard);
        assert!(
            state.begin_sync().is_ok(),
            "the flag clears when the sync ends"
        );
    }

    #[test]
    fn hotkey_error_round_trips_into_settings() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path(), &[row(1, "Hi-Potion")]);

        assert_eq!(state.settings().unwrap().hotkey_error, None);
        state.set_hotkey_error(Some("already in use".into()));
        assert_eq!(
            state.settings().unwrap().hotkey_error.as_deref(),
            Some("already in use")
        );

        state.set_hotkey_error(None);
        assert_eq!(state.settings().unwrap().hotkey_error, None);
    }
}
