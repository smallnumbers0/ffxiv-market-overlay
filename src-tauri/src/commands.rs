//! Every `#[tauri::command]` the frontend can call.
//!
//! Commands stay thin: validate, delegate to a module, return a serialisable
//! value or an `AppError`. Anything with real logic in it belongs in
//! `search.rs` / `universalis.rs` / `config.rs` so it can be unit-tested
//! without a running app.

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::config::AppConfig;
use crate::db::{CatalogInfo, Item};
use crate::error::{AppError, AppResult};
use crate::search::{SearchResult, DEFAULT_LIMIT};
use crate::state::AppState;
use crate::universalis::{DataCenter, PriceData, World};
use crate::xivapi_sync::{self, SyncProgress, SyncSummary};

/// Event name for catalog-sync progress. Mirrored in `src/lib/tauriApi.ts`.
pub const SYNC_PROGRESS_EVENT: &str = "catalog-sync-progress";

/// Everything the settings panel needs, in one round trip.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(flatten)]
    pub config: AppConfig,
    pub catalog: CatalogInfo,
    /// False when no catalog has been synced yet - the UI shows a setup
    /// prompt rather than an empty, apparently broken search box.
    pub catalog_ready: bool,
    /// Why the hotkey isn't working, or `None` when it is.
    pub hotkey_error: Option<String>,
}

/// The world/DC picker's options.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketScopes {
    pub worlds: Vec<World>,
    pub data_centers: Vec<DataCenter>,
}

#[tauri::command]
pub fn search_items(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<SearchResult>> {
    let index = state.search_index()?;
    Ok(index.search(&query, limit.unwrap_or(DEFAULT_LIMIT)))
}

/// Current prices for one item on the configured world/DC.
///
/// Served from the in-memory cache when fresh; `refresh: true` forces a
/// network round trip (the UI's manual refresh).
#[tauri::command]
pub async fn get_price(
    state: State<'_, AppState>,
    item_id: u32,
    refresh: Option<bool>,
) -> AppResult<PriceData> {
    let config = state.config.get();
    let scope = config.require_scope()?.to_string();

    if refresh.unwrap_or(false) {
        state.cache.invalidate(item_id, &scope);
    } else if let Some(cached) = state.cache.get(item_id, &scope) {
        return Ok(cached);
    }

    let price = state.universalis.fetch_price(item_id, &scope).await?;
    state.cache.insert(&scope, price.clone());
    Ok(price)
}

/// Mark an item as recently viewed.
///
/// Deliberately separate from `get_price`: the UI also fetches prices ahead of
/// a click for whatever row is highlighted, and a row the user merely moused
/// over must not end up in their recents. A read doesn't mutate user state.
#[tauri::command]
pub fn record_recent_item(state: State<'_, AppState>, item_id: u32) -> AppResult<()> {
    state.config.push_recent(item_id)?;
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.settings()
}

/// Seed a market scope on first launch so the app is usable the instant it
/// opens, instead of demanding a setup step before it will show a single price.
///
/// The frontend derives `region` from the system time zone - the only signal
/// available to a companion app that never reads the game. It's a guess, it is
/// always visible in the title bar, and the UI nudges the user to narrow it to
/// their own world. Calling this when a scope already exists does nothing.
#[tauri::command]
pub fn ensure_market_scope(state: State<'_, AppState>, region: String) -> AppResult<Settings> {
    state.config.set_guessed_market_scope(&region)?;
    state.settings()
}

/// Set the home world / data center. Clears the price cache, since every
/// entry in it belongs to the previous board.
#[tauri::command]
pub fn set_market_scope(state: State<'_, AppState>, scope: String) -> AppResult<Settings> {
    state.config.set_market_scope(&scope)?;
    state.cache.clear();
    state.settings()
}

/// Change the overlay hotkey, re-registering it immediately. If the new
/// accelerator can't be registered the old one is restored, so the user is
/// never left with no way to open the overlay.
#[tauri::command]
pub fn set_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> AppResult<Settings> {
    let previous = state.config.get().hotkey;
    // Re-applying the same accelerator is a no-op *unless* it isn't currently
    // registered - then it's the user retrying after freeing it up elsewhere.
    if previous == hotkey.trim() && state.hotkey_error().is_none() {
        return state.settings();
    }

    state.config.set_hotkey(&hotkey)?;
    match crate::hotkey::register(&app, &hotkey) {
        Ok(()) => {
            crate::hotkey::unregister(&app, &previous);
            state.set_hotkey_error(None);
            state.settings()
        }
        Err(error) => {
            // Put the working hotkey back rather than leaving the user with
            // neither the old one nor the new one.
            state.config.set_hotkey(&previous)?;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn list_market_scopes(state: State<'_, AppState>) -> AppResult<MarketScopes> {
    let (worlds, data_centers) = tokio::try_join!(
        state.universalis.fetch_worlds(),
        state.universalis.fetch_data_centers()
    )?;
    Ok(MarketScopes {
        worlds,
        data_centers,
    })
}

/// Recently viewed items, resolved to full catalog entries. IDs that are no
/// longer in the catalog (a patch removed them) are dropped silently.
#[tauri::command]
pub fn get_recent_items(state: State<'_, AppState>) -> AppResult<Vec<Item>> {
    let index = state.search_index()?;
    Ok(state
        .config
        .get()
        .recent_item_ids
        .iter()
        .filter_map(|id| index.get(*id).cloned())
        .collect())
}

#[tauri::command]
pub fn clear_recent_items(state: State<'_, AppState>) -> AppResult<()> {
    state.config.clear_recents()?;
    Ok(())
}

/// Re-download the catalog into the app-data copy, then swap the in-memory
/// index over. The bundled read-only resource is never touched.
#[tauri::command]
pub async fn refresh_catalog(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<SyncSummary> {
    let _guard = state.begin_sync()?;

    let summary = xivapi_sync::sync_catalog(&state.catalog_path, |progress| {
        let _ = app.emit(SYNC_PROGRESS_EVENT, progress_payload(progress));
    })
    .await?;

    state.reload_search_index()?;
    state.cache.clear();
    Ok(summary)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SyncProgressPayload {
    stage: &'static str,
    count: usize,
}

fn progress_payload(progress: SyncProgress) -> SyncProgressPayload {
    match progress {
        SyncProgress::FetchedItems(count) => SyncProgressPayload {
            stage: "items",
            count,
        },
        SyncProgress::FetchedMarketable(count) => SyncProgressPayload {
            stage: "marketable",
            count,
        },
        SyncProgress::Wrote(count) => SyncProgressPayload {
            stage: "wrote",
            count,
        },
    }
}

/// Hide the overlay (Escape, or the close button). Hiding rather than closing
/// keeps the app alive for the next hotkey press.
#[tauri::command]
pub fn hide_overlay(app: tauri::AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(crate::MAIN_WINDOW)
        .ok_or_else(|| AppError::Internal("overlay window is gone".into()))?;
    crate::save_geometry(&window.as_ref().window());
    window
        .hide()
        .map_err(|e| AppError::Internal(format!("could not hide the overlay: {e}")))
}

/// Where the app keeps its files, for the troubleshooting section of the UI.
#[tauri::command]
pub fn get_catalog_path(state: State<'_, AppState>) -> String {
    state.catalog_path.display().to_string()
}
