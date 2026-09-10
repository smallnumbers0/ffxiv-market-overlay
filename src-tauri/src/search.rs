//! In-memory fuzzy search over the marketable item catalog.
//!
//! The whole corpus (~17k marketable items) lives in RAM and every keystroke
//! is scored here in Rust. Nothing crosses the JS bridge except the handful of
//! results actually shown, and nothing touches SQLite after startup.

use std::sync::Mutex;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32String};
use serde::Serialize;

use crate::db::Item;

/// Cap on results handed to the UI. The list is ranked, so anything past the
/// first screenful is noise - and a short list keeps the bridge payload small.
pub const DEFAULT_LIMIT: usize = 30;

/// Item names are short human phrases, not file paths, and players type the
/// beginning of the name they are after. `prefer_prefix` biases the score
/// towards matches that start early in the name, which is what makes "hi-po"
/// rank *Hi-Potion* over *Ice Ward Hi-Potion*.
const MATCHER_CONFIG: Config = {
    let mut config = Config::DEFAULT;
    config.prefer_prefix = true;
    config
};

/// How many characters of name are worth one point of match score.
///
/// Raw scores separate a base item from its variants by only a point or two
/// ("Hi-Potion" 226 vs "Hi-Potion of Mind" 227 for the query "hi potion") -
/// noise from word-boundary bonuses, not a real difference in relevance. At
/// that resolution the shorter name is the better signal: FFXIV names the base
/// item plainly and appends qualifiers to its variants, and a player typing a
/// short query is usually after the base item.
///
/// Divided rather than subtracted outright so length stays a tie-break between
/// near-equal matches and never overturns a genuinely better one.
const CHARS_PER_SCORE_POINT: usize = 8;

fn adjusted_score(score: u32, name: &str) -> u32 {
    score.saturating_sub((name.chars().count() / CHARS_PER_SCORE_POINT) as u32)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(flatten)]
    pub item: Item,
    /// Higher is a better match. Exposed so the UI can highlight a runaway
    /// best match, not for display.
    pub score: u32,
}

pub struct SearchIndex {
    items: Vec<Item>,
    /// Pre-encoded haystacks, parallel to `items`. Encoding once at load
    /// instead of per keystroke is the difference between a few milliseconds
    /// and tens of milliseconds per query.
    haystacks: Vec<Utf32String>,
    /// `Matcher` carries reusable scratch buffers and needs `&mut`; one shared
    /// instance avoids reallocating them on every keystroke.
    matcher: Mutex<Matcher>,
}

impl SearchIndex {
    pub fn new(items: Vec<Item>) -> Self {
        let haystacks = items
            .iter()
            .map(|item| Utf32String::from(item.name.as_str()))
            .collect();
        Self {
            items,
            haystacks,
            matcher: Mutex::new(Matcher::new(MATCHER_CONFIG)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Rank the catalog against `query`. An empty query matches nothing -
    /// showing 17k items in arbitrary order helps no one.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let Ok(mut matcher) = self.matcher.lock() else {
            return Vec::new();
        };

        let mut scored: Vec<(u32, usize)> = self
            .haystacks
            .iter()
            .enumerate()
            .filter_map(|(index, haystack)| {
                pattern
                    .score(haystack.slice(..), &mut matcher)
                    .map(|score| (score, index))
            })
            .collect();

        // Best adjusted score first, then alphabetical for a stable order.
        scored.sort_by(|a, b| {
            let (a_score, a_name) = (a.0, &self.items[a.1].name);
            let (b_score, b_name) = (b.0, &self.items[b.1].name);
            adjusted_score(b_score, b_name)
                .cmp(&adjusted_score(a_score, a_name))
                .then_with(|| a_name.len().cmp(&b_name.len()))
                .then_with(|| a_name.cmp(b_name))
        });

        scored
            .into_iter()
            .take(limit)
            .map(|(score, index)| SearchResult {
                item: self.items[index].clone(),
                score,
            })
            .collect()
    }

    /// Exact-ish lookup used when the frontend asks for an item it already
    /// has an ID for (a recent search, a watchlist entry).
    pub fn get(&self, item_id: u32) -> Option<&Item> {
        self.items.iter().find(|item| item.item_id == item_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, name: &str) -> Item {
        Item {
            item_id: id,
            name: name.to_string(),
            icon_path: None,
            level_item: None,
            category_name: None,
        }
    }

    fn index() -> SearchIndex {
        SearchIndex::new(vec![
            item(1, "Potion"),
            item(2, "Hi-Potion"),
            item(3, "Mega-Potion"),
            item(4, "Potion of Dexterity"),
            item(5, "Cobalt Ingot"),
            item(6, "Grade 8 Tincture of Strength"),
            item(7, "Rarefied Cobalt Saw"),
        ])
    }

    fn names(results: &[SearchResult]) -> Vec<&str> {
        results.iter().map(|r| r.item.name.as_str()).collect()
    }

    #[test]
    fn exact_name_ranks_first() {
        let results = index().search("Potion", DEFAULT_LIMIT);
        assert_eq!(names(&results)[0], "Potion");
    }

    #[test]
    fn hyphenated_query_finds_its_item() {
        let results = index().search("hi-potion", DEFAULT_LIMIT);
        assert_eq!(names(&results)[0], "Hi-Potion");
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(
            names(&index().search("COBALT INGOT", DEFAULT_LIMIT))[0],
            "Cobalt Ingot"
        );
    }

    #[test]
    fn partial_and_abbreviated_queries_match() {
        assert_eq!(
            names(&index().search("cobin", DEFAULT_LIMIT))[0],
            "Cobalt Ingot"
        );
        assert_eq!(
            names(&index().search("tinc str", DEFAULT_LIMIT))[0],
            "Grade 8 Tincture of Strength"
        );
    }

    #[test]
    fn a_base_item_outranks_its_longer_variants() {
        // The real catalog scores "Hi-Potion" one point *below* "Hi-Potion of
        // Mind" for the query "hi potion" - a word-boundary artefact. At that
        // resolution the shorter name has to win.
        let index = SearchIndex::new(vec![
            item(1, "Hi-Potion of Mind"),
            item(2, "Hi-Potion"),
            item(3, "Hi-Potion of Intelligence"),
        ]);
        assert_eq!(
            names(&index.search("hi potion", DEFAULT_LIMIT))[0],
            "Hi-Potion"
        );
    }

    #[test]
    fn length_never_overturns_a_clearly_better_match() {
        let index = SearchIndex::new(vec![item(1, "Cobalt Ingot"), item(2, "Cob")]);
        // "Cob" is far shorter but a much worse match for the full query.
        assert_eq!(
            names(&index.search("cobalt ingot", DEFAULT_LIMIT))[0],
            "Cobalt Ingot"
        );
    }

    #[test]
    fn shorter_name_wins_a_score_tie() {
        let results = index().search("potion", DEFAULT_LIMIT);
        let potion = names(&results).iter().position(|n| *n == "Potion").unwrap();
        let long = names(&results)
            .iter()
            .position(|n| *n == "Potion of Dexterity")
            .unwrap();
        assert!(potion < long);
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(index().search("", DEFAULT_LIMIT).is_empty());
        assert!(index().search("   ", DEFAULT_LIMIT).is_empty());
    }

    #[test]
    fn nonsense_query_returns_nothing() {
        assert!(index().search("zzzzqqqxxx", DEFAULT_LIMIT).is_empty());
    }

    #[test]
    fn limit_is_honoured() {
        assert_eq!(index().search("o", 2).len(), 2);
        assert!(index().search("o", 0).is_empty());
    }

    #[test]
    fn get_finds_by_id() {
        let index = index();
        assert_eq!(index.get(5).unwrap().name, "Cobalt Ingot");
        assert!(index.get(999).is_none());
    }

    #[test]
    fn scoring_the_full_corpus_stays_fast() {
        // Guards the "no perceptible lag" acceptance criterion against a
        // regression to per-keystroke re-encoding or a SQLite round trip.
        let items: Vec<Item> = (0..20_000)
            .map(|i| item(i, &format!("Synthetic Item Number {i}")))
            .collect();
        let index = SearchIndex::new(items);

        let start = std::time::Instant::now();
        for query in ["syn", "synth", "synthetic", "item 1234", "sin19"] {
            index.search(query, DEFAULT_LIMIT);
        }
        // Deliberately loose: this is a regression guard against re-encoding
        // haystacks per keystroke or reaching back into SQLite, not a
        // benchmark. A release build is roughly an order of magnitude faster
        // than the debug build this usually runs under.
        let per_query = start.elapsed() / 5;
        assert!(
            per_query < std::time::Duration::from_millis(250),
            "search took {per_query:?} per query on a 20k corpus"
        );
        println!("search: {per_query:?} per query over 20k items (debug build)");
    }
}
