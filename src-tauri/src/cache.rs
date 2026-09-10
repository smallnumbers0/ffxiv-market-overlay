//! In-memory TTL cache for price lookups.
//!
//! Per-session only - cleared on restart, never written to disk. Prices are
//! the one genuinely shared, fast-moving piece of data in the app, and
//! Universalis is already the source of truth for them; this cache exists to
//! keep re-selecting the same item instant, not to persist anything.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::universalis::PriceData;

/// How long a price snapshot stays fresh. Market boards move on the order of
/// minutes, and Universalis's own uploads are not instant, so a short TTL
/// costs accuracy nothing and makes repeat lookups free.
pub const DEFAULT_TTL: Duration = Duration::from_secs(180);

/// A cached snapshot plus how stale it is.
pub struct Cached {
    pub price: PriceData,
    pub age: Duration,
}

impl Cached {
    pub fn is_fresh(&self, ttl: Duration) -> bool {
        self.age < ttl
    }
}

/// Key is (item, world-or-DC): the same item on two worlds is two entries.
type Key = (u32, String);

pub struct PriceCache {
    entries: RwLock<HashMap<Key, (Instant, PriceData)>>,
    ttl: Duration,
}

impl PriceCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Fresh entry only - `None` once past the TTL.
    pub fn get(&self, item_id: u32, scope: &str) -> Option<PriceData> {
        self.peek(item_id, scope)
            .filter(|c| c.is_fresh(self.ttl))
            .map(|c| c.price)
    }

    /// Any entry, however stale, with its age. Lets a caller paint known
    /// numbers immediately and refresh behind them.
    pub fn peek(&self, item_id: u32, scope: &str) -> Option<Cached> {
        let entries = self.entries.read().ok()?;
        let (stored_at, price) = entries.get(&key(item_id, scope))?;
        Some(Cached {
            price: price.clone(),
            age: stored_at.elapsed(),
        })
    }

    pub fn insert(&self, scope: &str, price: PriceData) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(key(price.item_id, scope), (Instant::now(), price));
        }
    }

    /// Drop one entry so the next lookup goes to the network (the UI's
    /// explicit "refresh" action).
    pub fn invalidate(&self, item_id: u32, scope: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(&key(item_id, scope));
        }
    }

    /// Drop everything - used when the selected world/DC changes, since every
    /// entry is scoped to the old one.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for PriceCache {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

/// World names are compared case-insensitively so "cactuar" and "Cactuar"
/// don't become two entries for the same board.
fn key(item_id: u32, scope: &str) -> Key {
    (item_id, scope.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(item_id: u32) -> PriceData {
        PriceData {
            item_id,
            scope: "Cactuar".into(),
            listings: Vec::new(),
            recent_sales: Vec::new(),
            min_price_nq: Some(100),
            min_price_hq: None,
            average_price_nq: None,
            average_price_hq: None,
            sale_velocity: 0.0,
            units_for_sale: None,
            units_sold: None,
            last_upload_time: None,
            fetched_at: 0,
        }
    }

    #[test]
    fn hits_within_ttl_and_misses_after() {
        // Margins are wide on purpose: a tight sleep here turns into a
        // flaky test the moment the machine is busy compiling.
        let cache = PriceCache::new(Duration::from_millis(150));
        cache.insert("Cactuar", price(1));
        assert!(cache.get(1, "Cactuar").is_some());

        std::thread::sleep(Duration::from_millis(400));
        assert!(cache.get(1, "Cactuar").is_none(), "expired");
        assert!(
            cache.peek(1, "Cactuar").is_some(),
            "still peekable when stale"
        );
    }

    #[test]
    fn scopes_are_separate_but_case_insensitive() {
        let cache = PriceCache::new(DEFAULT_TTL);
        cache.insert("Cactuar", price(1));
        assert!(cache.get(1, "cactuar").is_some());
        assert!(cache.get(1, "Aether").is_none());
        assert!(cache.get(2, "Cactuar").is_none());
    }

    #[test]
    fn invalidate_and_clear_drop_entries() {
        let cache = PriceCache::new(DEFAULT_TTL);
        cache.insert("Cactuar", price(1));
        cache.insert("Cactuar", price(2));
        cache.invalidate(1, "Cactuar");
        assert!(cache.get(1, "Cactuar").is_none());
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }
}
