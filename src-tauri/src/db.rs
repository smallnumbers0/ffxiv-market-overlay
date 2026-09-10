//! SQLite catalog: schema, load, and bulk replace.
//!
//! The catalog is static per game patch, so SQLite is only ever the
//! *startup-load* source of truth. Search runs against an in-memory copy
//! (see `search.rs`); nothing here is on a per-keystroke path.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Bump when the `items` / `sync_meta` shape changes so an old app-data copy
/// is rebuilt from the bundled resource instead of being read with the wrong
/// column set.
pub const SCHEMA_VERSION: i64 = 1;

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS items (
    item_id       INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    icon_path     TEXT,
    level_item    INTEGER,
    category_name TEXT,
    marketable    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_items_name ON items(name);

CREATE TABLE IF NOT EXISTS sync_meta (
    key   TEXT PRIMARY KEY,
    value TEXT
);
"#;

/// One catalog row. Serialised straight to the frontend as a search result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub item_id: u32,
    pub name: String,
    pub icon_path: Option<String>,
    pub level_item: Option<u32>,
    pub category_name: Option<String>,
}

/// What the last sync recorded, for display in the settings panel.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInfo {
    pub total_items: u32,
    pub marketable_items: u32,
    pub last_synced_at: Option<String>,
    pub last_synced_game_version: Option<String>,
}

/// Open (creating if absent) a catalog database with the schema applied.
pub fn open_read_write(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // WAL keeps a concurrent read (search reload) from blocking a sync write.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA_SQL)?;
    set_meta(&conn, "schema_version", &SCHEMA_VERSION.to_string())?;
    Ok(conn)
}

/// Open an existing catalog for reading only. Fails if it isn't there.
pub fn open_read_only(path: &Path) -> AppResult<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| AppError::Catalog(format!("cannot open {}: {e}", path.display())))
}

/// Every marketable item, name-ordered. This is the entire search corpus -
/// roughly 20k rows, a few MB in memory, loaded once at startup.
pub fn load_marketable_items(conn: &Connection) -> AppResult<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT item_id, name, icon_path, level_item, category_name
           FROM items
          WHERE marketable = 1
          ORDER BY name",
    )?;
    let items = stmt
        .query_map([], |row| {
            Ok(Item {
                item_id: row.get(0)?,
                name: row.get(1)?,
                icon_path: row.get(2)?,
                level_item: row.get(3)?,
                category_name: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn catalog_info(conn: &Connection) -> AppResult<CatalogInfo> {
    Ok(CatalogInfo {
        total_items: conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))?,
        marketable_items: conn.query_row(
            "SELECT COUNT(*) FROM items WHERE marketable = 1",
            [],
            |r| r.get(0),
        )?,
        last_synced_at: get_meta(conn, "last_synced_at")?,
        last_synced_game_version: get_meta(conn, "last_synced_game_version")?,
    })
}

/// Replace the whole `items` table in one transaction. A sync either lands
/// completely or leaves the previous catalog untouched - never a half-written
/// catalog that search would silently return holes from.
pub fn replace_items(conn: &mut Connection, items: &[ItemRow]) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM items", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO items
                 (item_id, name, icon_path, level_item, category_name, marketable)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for item in items {
            stmt.execute(rusqlite::params![
                item.item_id,
                item.name,
                item.icon_path,
                item.level_item,
                item.category_name,
                item.marketable as i64,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// A row on its way *into* the catalog. Unlike `Item` it carries `marketable`,
/// which the app never needs after the load-time filter.
#[derive(Debug, Clone)]
pub struct ItemRow {
    pub item_id: u32,
    pub name: String,
    pub icon_path: Option<String>,
    pub level_item: Option<u32>,
    pub category_name: Option<String>,
    pub marketable: bool,
}

pub fn get_meta(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM sync_meta WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    Ok(match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Does this file look like a usable catalog at the current schema version?
pub fn is_usable_catalog(path: &Path) -> bool {
    let Ok(conn) = open_read_only(path) else {
        return false;
    };
    let version = get_meta(&conn, "schema_version")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok());
    if version != Some(SCHEMA_VERSION) {
        return false;
    }
    conn.query_row("SELECT COUNT(*) FROM items WHERE marketable = 1", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| n > 0)
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u32, name: &str, marketable: bool) -> ItemRow {
        ItemRow {
            item_id: id,
            name: name.to_string(),
            icon_path: Some(format!("ui/icon/{id:06}.tex")),
            level_item: Some(1),
            category_name: Some("Medicine".into()),
            marketable,
        }
    }

    #[test]
    fn replace_items_round_trips_and_filters_unmarketable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("items.db");
        let mut conn = open_read_write(&path).unwrap();

        replace_items(
            &mut conn,
            &[
                row(1, "Hi-Potion", true),
                row(2, "Quest Item", false),
                row(3, "Cobalt Ingot", true),
            ],
        )
        .unwrap();

        let items = load_marketable_items(&conn).unwrap();
        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["Cobalt Ingot", "Hi-Potion"],
            "only marketable rows, name-ordered"
        );

        let info = catalog_info(&conn).unwrap();
        assert_eq!((info.total_items, info.marketable_items), (3, 2));
    }

    #[test]
    fn replace_items_is_atomic_across_syncs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("items.db");
        let mut conn = open_read_write(&path).unwrap();

        replace_items(&mut conn, &[row(1, "Old Item", true)]).unwrap();
        replace_items(&mut conn, &[row(2, "New Item", true)]).unwrap();

        let items = load_marketable_items(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "New Item");
    }

    #[test]
    fn is_usable_catalog_rejects_empty_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.db");
        assert!(!is_usable_catalog(&missing));

        let empty = dir.path().join("empty.db");
        open_read_write(&empty).unwrap();
        assert!(
            !is_usable_catalog(&empty),
            "schema but no rows is not usable"
        );

        let mut conn = open_read_write(&empty).unwrap();
        replace_items(&mut conn, &[row(1, "Hi-Potion", true)]).unwrap();
        drop(conn);
        assert!(is_usable_catalog(&empty));
    }

    #[test]
    fn meta_upserts() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_read_write(&dir.path().join("items.db")).unwrap();
        assert_eq!(get_meta(&conn, "last_synced_at").unwrap(), None);
        set_meta(&conn, "last_synced_at", "2026-01-01T00:00:00Z").unwrap();
        set_meta(&conn, "last_synced_at", "2026-02-02T00:00:00Z").unwrap();
        assert_eq!(
            get_meta(&conn, "last_synced_at").unwrap().as_deref(),
            Some("2026-02-02T00:00:00Z")
        );
    }
}
