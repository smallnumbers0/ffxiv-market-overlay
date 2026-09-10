//! Acceptance checks against the real generated catalog.
//!
//! `resources/items.db` is a build artifact, so these skip themselves when it
//! hasn't been synced yet (a fresh clone, or CI without network). Run
//! `npm run sync-catalog` first to exercise them.

use std::path::PathBuf;
use std::time::Instant;

use ffxiv_market_overlay_lib::db;
use ffxiv_market_overlay_lib::search::{SearchIndex, DEFAULT_LIMIT};

fn catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("items.db")
}

/// `None` (and a printed note) when there is no synced catalog to test.
fn load_index() -> Option<SearchIndex> {
    let path = catalog_path();
    if !db::is_usable_catalog(&path) {
        eprintln!(
            "skipping: no synced catalog at {} - run `npm run sync-catalog`",
            path.display()
        );
        return None;
    }
    let conn = db::open_read_only(&path).ok()?;
    Some(SearchIndex::new(db::load_marketable_items(&conn).ok()?))
}

#[test]
fn catalog_has_a_plausible_number_of_marketable_items() {
    let Some(index) = load_index() else { return };
    // Universalis lists a little under 17k tradable items today. Wide bounds:
    // this is a "did the sync silently truncate" check, not a fixed count.
    assert!(
        (10_000..40_000).contains(&index.len()),
        "{} marketable items looks wrong",
        index.len()
    );
}

#[test]
fn well_known_items_rank_first() {
    let Some(index) = load_index() else { return };

    // (query, expected top result) - the Phase 2 acceptance examples plus a
    // few shapes real users type: exact, hyphenated, abbreviated, misspelled.
    let cases = [
        ("hi-potion", "Hi-Potion"),
        ("hipotion", "Hi-Potion"),
        ("hi potion", "Hi-Potion"),
        ("potion", "Potion"),
        ("cobalt ingot", "Cobalt Ingot"),
        ("cobalt ing", "Cobalt Ingot"),
        ("earth shard", "Earth Shard"),
        ("savage aim materia x", "Savage Aim Materia X"),
        ("dark matter cluster", "Dark Matter Cluster"),
    ];

    for (query, expected) in cases {
        let results = index.search(query, DEFAULT_LIMIT);
        let top = results.first().map(|r| r.item.name.as_str());
        assert_eq!(top, Some(expected), "query {query:?} ranked {top:?} first");
    }
}

#[test]
fn a_partial_query_surfaces_the_item_within_a_few_keystrokes() {
    let Some(index) = load_index() else { return };

    // Abbreviations a player would actually type. These don't always take the
    // top slot - "cobin" is a slightly better literal match for "Cob Inner
    // Wall" than for "Cobalt Ingot" - but the item has to be on screen.
    let cases = [
        ("hi-po", "Hi-Potion"),
        ("cobin", "Cobalt Ingot"),
        ("savaim x", "Savage Aim Materia X"),
    ];

    for (query, expected) in cases {
        let results = index.search(query, DEFAULT_LIMIT);
        let rank = results.iter().position(|r| r.item.name == expected);
        assert!(
            matches!(rank, Some(position) if position < 5),
            "{expected:?} ranked {rank:?} for {query:?}"
        );
    }
}

#[test]
fn untradable_items_never_appear() {
    let Some(index) = load_index() else { return };

    // Gil (1) and Allagan Tomestone of Causality (44) are not marketable.
    assert!(index.get(1).is_none(), "Gil must not be searchable");
    assert!(index.get(44).is_none(), "tomestones must not be searchable");
    assert!(
        index
            .search("gil", DEFAULT_LIMIT)
            .iter()
            .all(|r| r.item.item_id != 1),
        "searching 'gil' must not return the currency"
    );
}

#[test]
fn search_over_the_real_catalog_is_fast_enough_to_type_into() {
    let Some(index) = load_index() else { return };

    let queries = ["h", "hi", "hi-", "hi-p", "hi-po", "hi-pot", "hi-potion"];
    let start = Instant::now();
    for query in queries {
        index.search(query, DEFAULT_LIMIT);
    }
    let per_keystroke = start.elapsed() / queries.len() as u32;

    println!(
        "real catalog: {} items, {per_keystroke:?} per keystroke",
        index.len()
    );
    // Loose enough for a debug build on a loaded machine; a release build is
    // roughly an order of magnitude under this.
    assert!(
        per_keystroke < std::time::Duration::from_millis(250),
        "{per_keystroke:?} per keystroke over {} items",
        index.len()
    );
}
