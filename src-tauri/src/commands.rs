//! Every `#[tauri::command]` the frontend can call.
//!
//! Commands stay thin: validate, delegate to a module, return a serialisable
//! value or an `AppError`. Anything with real logic in it belongs in
//! `search.rs` / `universalis.rs` / `config.rs` so it can be unit-tested
//! without a running app.

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::db::{CatalogInfo, Item};
use crate::error::{AppError, AppResult};
use crate::search::{SearchResult, DEFAULT_LIMIT};
use crate::state::AppState;
use crate::universalis::{MarketScopes, PriceData};
use crate::xivapi_sync::{self, SyncProgress, SyncSummary};

/// Event name for catalog-sync progress. Mirrored in `src/lib/tauriApi.ts`.
pub const SYNC_PROGRESS_EVENT: &str = "catalog-sync-progress";

/// How long adding a board will wait on the world list before giving up and
/// copying the current one instead. Adding a column must not hang on the
/// network - the user can always pick a board by hand.
const WIDEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// One board being compared, as the strip and its column need it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardView {
    pub id: String,
    /// This board's world or data center.
    pub scope: Option<String>,
}

/// Everything the UI needs, in one round trip.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Boards being compared, left to right - one column each.
    pub boards: Vec<BoardView>,
    /// False once `config::MAX_BOARDS` are open - the UI disables its "+".
    pub can_add_board: bool,
    pub hotkey: String,
    pub recent_item_ids: Vec<u32>,
    /// Hearted item ids, so a row or panel can draw a filled heart without a
    /// second round trip.
    pub favorite_item_ids: Vec<u32>,
    pub catalog: CatalogInfo,
    /// False when no catalog has been synced yet - the UI shows a setup
    /// prompt rather than an empty, apparently broken search box.
    pub catalog_ready: bool,
    /// Why the hotkey isn't working, or `None` when it is.
    pub hotkey_error: Option<String>,
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

/// Current prices for one item on one board.
///
/// The UI calls this once per column when an item is opened. Requests are
/// per-board because Universalis has no endpoint that spans worlds; the cache
/// is keyed by (item, board), so a board you already looked at answers without
/// touching the network.
#[tauri::command]
pub async fn get_price(
    state: State<'_, AppState>,
    board: String,
    item_id: u32,
    refresh: Option<bool>,
) -> AppResult<PriceData> {
    let config = state.config.get();
    let scope = config.require_board(&board)?.require_scope()?.to_string();

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
/// available to a companion app that never reads the game. A region is not
/// itself a usable board (whole-region queries time out; see
/// `MarketScopes::widen`), so it is resolved to one data center inside it.
/// The result is a guess and may well be the wrong data center, which is why
/// the board strip names it and opens the picker on click. It is not
/// announced: the guess costs a click to fix, a banner costs one from
/// everybody.
///
/// The same pass retires region scopes saved by earlier versions, which is the
/// only way those users get off a board that answers every lookup with a 504.
/// The replacement data center is as arbitrary as any first-launch guess, but
/// the board strip names it and one click changes it. Boards that already name
/// a world or data center are left alone.
///
/// Never fails on a cold world list: without it there is nothing to resolve
/// against, and a first launch with no network has no prices to show anyway.
#[tauri::command]
pub async fn ensure_market_scope(
    state: State<'_, AppState>,
    board: String,
    region: String,
) -> AppResult<Settings> {
    let Some(scopes) = scopes_for_resolving(&state).await else {
        return state.settings();
    };

    // Regions saved by an older version, on any board - not just `board`.
    let stale: Vec<(String, String)> = state
        .config
        .get()
        .boards
        .iter()
        .filter_map(|b| {
            let scope = b.scope.as_deref()?;
            let dc = scopes.default_data_center(scope)?;
            Some((b.id.clone(), dc))
        })
        .collect();
    for (id, dc) in stale {
        state.config.set_board_scope(&id, &dc)?;
    }

    if let Some(dc) = scopes.default_data_center(&region) {
        state.config.set_guessed_board_scope(&board, &dc)?;
    }
    state.settings()
}

/// The world list, if it arrives quickly enough to be worth waiting on.
/// Resolving a scope is startup work - it must not hold the UI open.
async fn scopes_for_resolving<'a>(state: &'a State<'_, AppState>) -> Option<&'a MarketScopes> {
    match tokio::time::timeout(WIDEN_TIMEOUT, state.market_scopes()).await {
        Ok(Ok(scopes)) => Some(scopes),
        Ok(Err(error)) => {
            eprintln!("couldn't reach the world list to resolve a scope: {error}");
            None
        }
        Err(_) => {
            eprintln!("timed out fetching the world list to resolve a scope");
            None
        }
    }
}

/// Point one board at a world or data center. Only that column moves.
///
/// The price cache is left alone: entries are keyed by (item, board), so the
/// new board simply misses and fetches, and any other column still on the old
/// one keeps its warm entries.
#[tauri::command]
pub fn set_market_scope(
    state: State<'_, AppState>,
    board: String,
    scope: String,
) -> AppResult<Settings> {
    state.config.set_board_scope(&board, &scope)?;
    state.settings()
}

/// Add a column, one level wider than the rightmost board.
///
/// A second board is nearly always added to answer "is the rest of my data
/// center cheaper?", so it starts on the current board's data center and the
/// user can narrow it from there. A board already on a data center is as wide
/// as boards go, so the new column copies it. If the world list can't be
/// reached in time the column still opens - on the same board as its
/// neighbour.
#[tauri::command]
pub async fn add_board(state: State<'_, AppState>) -> AppResult<Settings> {
    let current = state
        .config
        .get()
        .boards
        .last()
        .and_then(|board| board.scope.clone());

    let scope = match current {
        Some(ref scope) => widen(&state, scope).await.or_else(|| current.clone()),
        None => None,
    };

    state.config.add_board(scope)?;
    state.settings()
}

/// Close a column. The last board stays - see `ConfigStore::remove_board`.
#[tauri::command]
pub fn remove_board(state: State<'_, AppState>, board: String) -> AppResult<Settings> {
    state.config.remove_board(&board)?;
    state.settings()
}

/// The board one level out from `scope`, or `None` if it can't be worked out
/// quickly. Never an error: adding a column is not worth failing over.
async fn widen(state: &AppState, scope: &str) -> Option<String> {
    match tokio::time::timeout(WIDEN_TIMEOUT, state.market_scopes()).await {
        Ok(Ok(scopes)) => scopes.widen(scope),
        Ok(Err(error)) => {
            eprintln!("couldn't widen '{scope}' for a new board: {error}");
            None
        }
        Err(_) => {
            eprintln!("timed out looking up the data center for '{scope}'");
            None
        }
    }
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
    Ok(state.market_scopes().await?.clone())
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

/// Heart an item, or un-heart one already hearted.
#[tauri::command]
pub fn toggle_favorite(state: State<'_, AppState>, item_id: u32) -> AppResult<Settings> {
    state.config.toggle_favorite(item_id)?;
    state.settings()
}

/// Hearted items, resolved to full catalog entries, newest first. Ids no
/// longer in the catalog (a patch removed them) are dropped silently, the same
/// way recents are.
#[tauri::command]
pub fn get_favorite_items(state: State<'_, AppState>) -> AppResult<Vec<Item>> {
    let index = state.search_index()?;
    Ok(state
        .config
        .get()
        .favorite_item_ids
        .iter()
        .filter_map(|id| index.get(*id).cloned())
        .collect())
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
