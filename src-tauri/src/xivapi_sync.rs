//! Builds `items.db` from XIVAPI v2 (names, icons, categories) plus
//! Universalis (which of those items are actually tradable).
//!
//! Runs at build time via the `sync_catalog` binary, and on demand from the
//! app's "refresh item database" action. Both paths call `sync_catalog`.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use serde::Deserialize;

use crate::db::{self, ItemRow};
use crate::error::{AppError, AppResult};
use crate::universalis::{now_millis, UniversalisClient};

const XIVAPI_BASE: &str = "https://v2.xivapi.com";
const USER_AGENT: &str = concat!("ffxiv-market-overlay/", env!("CARGO_PKG_VERSION"));

/// XIVAPI caps `limit` at 500.
const PAGE_SIZE: u32 = 500;
/// Generous next to the app's 5s budget: a sync is a deliberate, one-off,
/// user-initiated operation, not something checked mid-fight.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Row IDs are sparse, so "fewer rows than asked for" is the only reliable
/// end-of-sheet signal. This is just a runaway guard.
const MAX_PAGES: u32 = 400;

/// Only these fields are requested. `@as(raw)` on `LevelItem` returns the item
/// level as a bare number instead of expanding the entire ItemLevel sheet row,
/// which is ~90 fields of stats we don't want.
const ITEM_FIELDS: &str = "Name,Icon.path,LevelItem@as(raw),ItemUICategory.Name";

/// Progress ticks, reported as the sync runs.
#[derive(Debug, Clone, Copy)]
pub enum SyncProgress {
    /// Items pulled from XIVAPI so far.
    FetchedItems(usize),
    /// Marketable IDs pulled from Universalis.
    FetchedMarketable(usize),
    /// Rows written to SQLite.
    Wrote(usize),
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub total_items: usize,
    pub marketable_items: usize,
}

/// Fetch the catalog and write it to `db_path`, replacing whatever was there.
///
/// The write happens in a single transaction at the very end, so a network
/// failure part-way leaves the previous catalog fully intact.
pub async fn sync_catalog<F>(db_path: &Path, mut on_progress: F) -> AppResult<SyncSummary>
where
    F: FnMut(SyncProgress),
{
    let client = XivApiClient::new()?;
    let universalis = UniversalisClient::new()?;

    let (items, game_version) = client.fetch_all_items(&mut on_progress).await?;

    let marketable = universalis.fetch_marketable_ids().await?;
    on_progress(SyncProgress::FetchedMarketable(marketable.len()));
    if marketable.is_empty() {
        return Err(AppError::Universalis(
            "marketable item list came back empty; refusing to write a catalog with no \
             tradable items"
                .into(),
        ));
    }
    let marketable: std::collections::HashSet<u32> = marketable.into_iter().collect();

    let rows: Vec<ItemRow> = items
        .into_iter()
        .map(|item| ItemRow {
            marketable: marketable.contains(&item.item_id),
            ..item
        })
        .collect();

    let summary = SyncSummary {
        total_items: rows.len(),
        marketable_items: rows.iter().filter(|r| r.marketable).count(),
    };

    let mut conn = db::open_read_write(db_path)?;
    db::replace_items(&mut conn, &rows)?;
    db::set_meta(&conn, "last_synced_at", &now_millis().to_string())?;
    if let Some(version) = game_version {
        db::set_meta(&conn, "last_synced_game_version", &version)?;
    }
    checkpoint_wal(&conn);
    on_progress(SyncProgress::Wrote(summary.total_items));

    Ok(summary)
}

/// Fold the WAL back into the main file so the shipped `items.db` is a single
/// self-contained artifact with no sidecar files.
fn checkpoint_wal(conn: &Connection) {
    let _ = conn.pragma_update(None, "journal_mode", "DELETE");
}

struct XivApiClient {
    http: reqwest::Client,
}

impl XivApiClient {
    fn new() -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client setup failed: {e}")))?;
        Ok(Self { http })
    }

    /// Walk the whole Item sheet. Row IDs are not contiguous, so pagination
    /// uses `after = last row_id seen` and stops on a short page.
    async fn fetch_all_items<F>(
        &self,
        on_progress: &mut F,
    ) -> AppResult<(Vec<ItemRow>, Option<String>)>
    where
        F: FnMut(SyncProgress),
    {
        let mut items: Vec<ItemRow> = Vec::with_capacity(50_000);
        let mut after: Option<u32> = None;
        let mut game_version: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let page = self.fetch_page(after).await?;
            game_version = game_version.or(page.version);

            let row_count = page.rows.len();
            if let Some(last) = page.rows.last() {
                after = Some(last.row_id);
            }

            items.extend(page.rows.into_iter().filter_map(row_to_item));
            on_progress(SyncProgress::FetchedItems(items.len()));

            if (row_count as u32) < PAGE_SIZE {
                return Ok((items, game_version));
            }
        }

        Err(AppError::XivApi(format!(
            "gave up after {MAX_PAGES} pages; the sheet is larger than expected"
        )))
    }

    async fn fetch_page(&self, after: Option<u32>) -> AppResult<SheetPage> {
        let mut url =
            format!("{XIVAPI_BASE}/api/sheet/Item?fields={ITEM_FIELDS}&limit={PAGE_SIZE}");
        if let Some(after) = after {
            url.push_str(&format!("&after={after}"));
        }

        let response = self.http.get(&url).send().await.map_err(|e| {
            AppError::XivApi(if e.is_timeout() {
                "request timed out".to_string()
            } else {
                e.to_string()
            })
        })?;

        if !response.status().is_success() {
            return Err(AppError::XivApi(format!(
                "server replied {}",
                response.status().as_u16()
            )));
        }

        response
            .json::<SheetPage>()
            .await
            .map_err(|e| AppError::XivApi(format!("unexpected response: {e}")))
    }
}

/// Drop rows with no name: the Item sheet is padded with blank placeholder
/// rows that can never appear on a market board.
fn row_to_item(row: SheetRow) -> Option<ItemRow> {
    let name = row.fields.name.trim();
    if name.is_empty() {
        return None;
    }
    Some(ItemRow {
        item_id: row.row_id,
        name: name.to_string(),
        icon_path: row
            .fields
            .icon
            .and_then(|i| i.path)
            .filter(|p| !p.is_empty()),
        level_item: row.fields.level_item,
        category_name: row
            .fields
            .category
            .map(|c| c.fields.name)
            .filter(|n| !n.trim().is_empty()),
        marketable: false,
    })
}

#[derive(Debug, Deserialize)]
struct SheetPage {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    rows: Vec<SheetRow>,
}

#[derive(Debug, Deserialize)]
struct SheetRow {
    row_id: u32,
    fields: SheetFields,
}

#[derive(Debug, Deserialize)]
struct SheetFields {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Icon", default)]
    icon: Option<IconField>,
    #[serde(rename = "LevelItem@as(raw)", default)]
    level_item: Option<u32>,
    #[serde(rename = "ItemUICategory", default)]
    category: Option<CategoryField>,
}

#[derive(Debug, Deserialize)]
struct IconField {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CategoryField {
    fields: CategoryName,
}

#[derive(Debug, Deserialize)]
struct CategoryName {
    #[serde(rename = "Name", default)]
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"{
      "schema": "exdschema@2",
      "version": "f5af21155b99a524",
      "rows": [
        {"row_id": 5, "fields": {
          "Icon": {"id": 20006, "path": "ui/icon/020000/020006.tex"},
          "ItemUICategory": {"value": 59, "row_id": 59, "fields": {"Name": "Crystal"}},
          "LevelItem@as(raw)": 1,
          "Name": "Earth Shard"}},
        {"row_id": 6, "fields": {
          "Icon": {"id": 0, "path": ""},
          "ItemUICategory": {"value": 0, "row_id": 0, "fields": {"Name": ""}},
          "LevelItem@as(raw)": 0,
          "Name": ""}}
      ]
    }"#;

    #[test]
    fn parses_a_sheet_page_and_drops_blank_rows() {
        let page: SheetPage = serde_json::from_str(PAGE).unwrap();
        assert_eq!(page.version.as_deref(), Some("f5af21155b99a524"));
        assert_eq!(page.rows.len(), 2);

        let items: Vec<ItemRow> = page.rows.into_iter().filter_map(row_to_item).collect();
        assert_eq!(items.len(), 1, "the unnamed placeholder row is dropped");

        let shard = &items[0];
        assert_eq!(shard.item_id, 5);
        assert_eq!(shard.name, "Earth Shard");
        assert_eq!(
            shard.icon_path.as_deref(),
            Some("ui/icon/020000/020006.tex")
        );
        assert_eq!(shard.level_item, Some(1));
        assert_eq!(shard.category_name.as_deref(), Some("Crystal"));
        assert!(
            !shard.marketable,
            "marketability is applied later, from Universalis"
        );
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let page: SheetPage =
            serde_json::from_str(r#"{"rows":[{"row_id":9,"fields":{"Name":"Mystery"}}]}"#).unwrap();
        let item = row_to_item(page.rows.into_iter().next().unwrap()).unwrap();
        assert_eq!(item.name, "Mystery");
        assert_eq!(item.icon_path, None);
        assert_eq!(item.level_item, None);
        assert_eq!(item.category_name, None);
    }

    #[test]
    fn empty_strings_become_none_not_empty_strings() {
        let page: SheetPage = serde_json::from_str(PAGE).unwrap();
        let mut rows = page.rows.into_iter();
        rows.next();
        assert!(row_to_item(rows.next().unwrap()).is_none());
    }
}
