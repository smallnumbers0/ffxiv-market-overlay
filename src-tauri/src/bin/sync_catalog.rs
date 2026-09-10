//! Build-time catalog generator.
//!
//! Usage:
//!     npm run sync-catalog                 # writes src-tauri/resources/items.db
//!     npm run sync-catalog -- /some/path.db
//!
//! Pulls every item from XIVAPI v2, flags the ones Universalis reports as
//! marketable, and writes the bundled `items.db`. Safe to re-run: the catalog
//! is replaced in one transaction, so a failed run leaves the old file intact.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use ffxiv_market_overlay_lib::db;
use ffxiv_market_overlay_lib::xivapi_sync::{sync_catalog, SyncProgress};

#[tokio::main]
async fn main() -> ExitCode {
    let db_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_db_path);

    println!("Syncing item catalog into {}", db_path.display());

    let result = sync_catalog(&db_path, |progress| match progress {
        SyncProgress::FetchedItems(count) => {
            print!("\r  XIVAPI: {count} items");
            let _ = std::io::stdout().flush();
        }
        SyncProgress::FetchedMarketable(count) => {
            println!("\n  Universalis: {count} marketable item IDs");
        }
        SyncProgress::Wrote(count) => println!("  Wrote {count} rows"),
    })
    .await;

    match result {
        Ok(summary) => {
            println!(
                "\nDone: {} items, {} marketable.",
                summary.total_items, summary.marketable_items
            );
            report_samples(&db_path);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("\nSync failed: {error}");
            eprintln!("The previous catalog (if any) was left untouched.");
            ExitCode::FAILURE
        }
    }
}

/// `resources/items.db` next to this crate's Cargo.toml, wherever it's run from.
fn default_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("items.db")
}

/// Print a few marketable names so a human can eyeball them against the game
/// or universalis.app - the Phase 1 acceptance check.
fn report_samples(db_path: &std::path::Path) {
    let Ok(conn) = db::open_read_only(db_path) else {
        return;
    };
    let Ok(items) = db::load_marketable_items(&conn) else {
        return;
    };
    println!("\nSample marketable items:");
    for item in items.iter().step_by((items.len() / 5).max(1)).take(5) {
        println!("  {:>7}  {}", item.item_id, item.name);
    }
}
