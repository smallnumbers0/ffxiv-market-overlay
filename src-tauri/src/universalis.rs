//! Universalis HTTP client.
//!
//! Hand-rolled on `reqwest` rather than the `universalis` crate: that crate is
//! at 0.1.0, last published 2024-03, and doesn't cover the endpoints we need.
//!
//! Every request carries a short timeout - this gets checked mid-gameplay, so
//! a failure has to surface fast and visibly rather than hang.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const BASE_URL: &str = "https://universalis.app/api/v2";
const USER_AGENT: &str = concat!("ffxiv-market-overlay/", env!("CARGO_PKG_VERSION"));
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How many rows to pull back per item. Small on purpose: the overlay shows a
/// handful of cheapest listings and recent sales, and a smaller body is a
/// faster body.
const LISTINGS_PER_ITEM: u32 = 12;
const HISTORY_ENTRIES_PER_ITEM: u32 = 12;

/// Universalis accepts at most 100 comma-separated IDs per request.
pub const MAX_IDS_PER_REQUEST: usize = 100;

#[derive(Clone)]
pub struct UniversalisClient {
    http: reqwest::Client,
    base_url: String,
}

impl UniversalisClient {
    pub fn new() -> AppResult<Self> {
        Self::with_base_url(BASE_URL)
    }

    /// Same client pointed at another base URL - used by the tests to talk to
    /// a local stub server instead of the real API.
    pub fn with_base_url(base_url: &str) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            // Keeping the connection warm removes a TLS handshake from the
            // second and later price lookups in a session.
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client setup failed: {e}")))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Current market state for one item on a world, DC, or region.
    pub async fn fetch_price(&self, item_id: u32, scope: &str) -> AppResult<PriceData> {
        let raw: RawMarketData = self.get_market(&[item_id], scope).await?;
        Ok(PriceData::from_raw(item_id, scope, raw))
    }

    /// Current market state for up to `MAX_IDS_PER_REQUEST` items at once.
    /// Items Universalis has no data for are simply absent from the result.
    pub async fn fetch_prices(&self, item_ids: &[u32], scope: &str) -> AppResult<Vec<PriceData>> {
        match item_ids {
            [] => Ok(Vec::new()),
            [single] => Ok(vec![self.fetch_price(*single, scope).await?]),
            many => {
                if many.len() > MAX_IDS_PER_REQUEST {
                    return Err(AppError::Invalid(format!(
                        "at most {MAX_IDS_PER_REQUEST} items per request, got {}",
                        many.len()
                    )));
                }
                let multi: RawMultiMarketData = self.get_market(many, scope).await?;
                // Preserve the caller's ordering rather than the map's.
                Ok(many
                    .iter()
                    .filter_map(|id| {
                        let raw = multi.items.get(&id.to_string())?.clone();
                        Some(PriceData::from_raw(*id, scope, raw))
                    })
                    .collect())
            }
        }
    }

    pub async fn fetch_worlds(&self) -> AppResult<Vec<World>> {
        self.get_json(&format!("{}/worlds", self.base_url)).await
    }

    pub async fn fetch_data_centers(&self) -> AppResult<Vec<DataCenter>> {
        self.get_json(&format!("{}/data-centers", self.base_url))
            .await
    }

    /// Item IDs that are actually tradable on the market board.
    pub async fn fetch_marketable_ids(&self) -> AppResult<Vec<u32>> {
        self.get_json(&format!("{}/marketable", self.base_url))
            .await
    }

    async fn get_market<T: serde::de::DeserializeOwned>(
        &self,
        item_ids: &[u32],
        scope: &str,
    ) -> AppResult<T> {
        let scope = scope.trim();
        if scope.is_empty() {
            return Err(AppError::Invalid("no world or data center selected".into()));
        }
        let ids = item_ids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "{}/{}/{}?listings={}&entries={}",
            self.base_url,
            urlencode(scope),
            ids,
            LISTINGS_PER_ITEM,
            HISTORY_ENTRIES_PER_ITEM
        );
        self.get_json(&url).await
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> AppResult<T> {
        let response = self.http.get(url).send().await.map_err(|e| {
            AppError::Universalis(if e.is_timeout() {
                "request timed out".to_string()
            } else if e.is_connect() {
                "no network connection".to_string()
            } else {
                e.to_string()
            })
        })?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::Universalis(
                "unknown world/data center, or no market data for that item".into(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::Universalis(format!(
                "server replied {}",
                status.as_u16()
            )));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| AppError::Universalis(format!("unexpected response: {e}")))
    }
}

/// Percent-encode the few characters a world/DC/region name could contain.
/// Names are ASCII words today; this keeps a stray space from breaking the URL.
fn urlencode(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            other => other
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

// --- Public, frontend-facing shapes -----------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceData {
    pub item_id: u32,
    /// The world / DC / region this was queried for.
    pub scope: String,
    pub listings: Vec<Listing>,
    pub recent_sales: Vec<Sale>,
    pub min_price_nq: Option<u32>,
    pub min_price_hq: Option<u32>,
    pub average_price_nq: Option<f64>,
    pub average_price_hq: Option<f64>,
    /// Sales per day, as reported by Universalis.
    pub sale_velocity: f64,
    pub units_for_sale: Option<u32>,
    pub units_sold: Option<u32>,
    /// Unix milliseconds of the most recent upload, if any.
    pub last_upload_time: Option<i64>,
    /// Unix milliseconds this snapshot was fetched, so the UI can show its age.
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub price_per_unit: u32,
    pub quantity: u32,
    pub total: u32,
    pub hq: bool,
    /// Present on data-center and region queries; `None` for a single world.
    pub world_name: Option<String>,
    pub retainer_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sale {
    pub price_per_unit: u32,
    pub quantity: u32,
    pub hq: bool,
    /// Unix seconds.
    pub timestamp: i64,
    pub world_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCenter {
    pub name: String,
    pub region: String,
    #[serde(default)]
    pub worlds: Vec<u32>,
}

/// Every board a user can pick, as one bundle. Fetched once per session and
/// memoised in `AppState` - the list only changes when Square Enix adds a
/// world, and both the settings picker and new panes need it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketScopes {
    pub worlds: Vec<World>,
    pub data_centers: Vec<DataCenter>,
}

impl MarketScopes {
    /// The next-widest board containing `scope`: a world widens to its data
    /// center, a data center to its region, and a region has nowhere to go.
    ///
    /// This is what makes a freshly opened pane immediately useful. The reason
    /// to want a second one is nearly always to ask "is the rest of my DC
    /// selling this cheaper?", so a new pane opens one level out from the one
    /// it was opened from rather than duplicating it.
    pub fn widen(&self, scope: &str) -> Option<String> {
        let scope = scope.trim();
        if scope.is_empty() {
            return None;
        }

        if let Some(world) = self.worlds.iter().find(|world| same(&world.name, scope)) {
            if let Some(dc) = self
                .data_centers
                .iter()
                .find(|dc| dc.worlds.contains(&world.id))
            {
                return Some(dc.name.clone());
            }
        }

        self.data_centers
            .iter()
            .find(|dc| same(&dc.name, scope))
            .map(|dc| dc.region.clone())
    }
}

/// Board names come from a dropdown built out of this same list, but a config
/// file can be hand-edited, so compare the way Universalis itself does.
fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

// --- Wire shapes ------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMultiMarketData {
    #[serde(default)]
    items: HashMap<String, RawMarketData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarketData {
    #[serde(default)]
    listings: Vec<RawListing>,
    #[serde(default)]
    recent_history: Vec<RawSale>,
    // NQ/HQ are uppercase on the wire; serde's camelCase rule would produce
    // `minPriceNq` and silently deserialise every one of these to `None`.
    #[serde(rename = "minPriceNQ", default)]
    min_price_nq: Option<f64>,
    #[serde(rename = "minPriceHQ", default)]
    min_price_hq: Option<f64>,
    #[serde(rename = "currentAveragePriceNQ", default)]
    current_average_price_nq: Option<f64>,
    #[serde(rename = "currentAveragePriceHQ", default)]
    current_average_price_hq: Option<f64>,
    #[serde(default)]
    regular_sale_velocity: Option<f64>,
    #[serde(default)]
    units_for_sale: Option<u32>,
    #[serde(default)]
    units_sold: Option<u32>,
    #[serde(default)]
    last_upload_time: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawListing {
    price_per_unit: u32,
    quantity: u32,
    #[serde(default)]
    total: Option<u32>,
    #[serde(default)]
    hq: bool,
    #[serde(default)]
    world_name: Option<String>,
    #[serde(default)]
    retainer_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSale {
    price_per_unit: u32,
    quantity: u32,
    #[serde(default)]
    hq: bool,
    #[serde(default)]
    timestamp: i64,
    #[serde(default)]
    world_name: Option<String>,
}

impl PriceData {
    fn from_raw(item_id: u32, scope: &str, raw: RawMarketData) -> Self {
        let mut listings: Vec<Listing> = raw
            .listings
            .into_iter()
            .map(|l| Listing {
                total: l.total.unwrap_or(l.price_per_unit * l.quantity),
                price_per_unit: l.price_per_unit,
                quantity: l.quantity,
                hq: l.hq,
                world_name: l.world_name,
                retainer_name: l.retainer_name.filter(|n| !n.is_empty()),
            })
            .collect();
        // Universalis usually returns these cheapest-first, but the overlay's
        // whole job is "what does this cost", so don't take that on faith.
        listings.sort_by_key(|l| l.price_per_unit);

        let mut recent_sales: Vec<Sale> = raw
            .recent_history
            .into_iter()
            .map(|s| Sale {
                price_per_unit: s.price_per_unit,
                quantity: s.quantity,
                hq: s.hq,
                timestamp: s.timestamp,
                world_name: s.world_name,
            })
            .collect();
        recent_sales.sort_by_key(|sale| std::cmp::Reverse(sale.timestamp));

        PriceData {
            item_id,
            scope: scope.to_string(),
            listings,
            recent_sales,
            // Universalis reports 0 (not null) when it has nothing; treat that
            // as "no data" so the UI shows a dash instead of a 0 gil price.
            min_price_nq: positive_u32(raw.min_price_nq),
            min_price_hq: positive_u32(raw.min_price_hq),
            average_price_nq: positive_f64(raw.current_average_price_nq),
            average_price_hq: positive_f64(raw.current_average_price_hq),
            sale_velocity: raw.regular_sale_velocity.unwrap_or(0.0),
            units_for_sale: raw.units_for_sale,
            units_sold: raw.units_sold,
            last_upload_time: raw.last_upload_time.filter(|t| *t > 0),
            fetched_at: now_millis(),
        }
    }

    /// True when Universalis has never seen this item on this world/DC.
    pub fn is_empty(&self) -> bool {
        self.listings.is_empty() && self.recent_sales.is_empty()
    }
}

fn positive_u32(value: Option<f64>) -> Option<u32> {
    value.filter(|v| *v > 0.0).map(|v| v.round() as u32)
}

fn positive_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|v| *v > 0.0)
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes() -> MarketScopes {
        MarketScopes {
            worlds: vec![
                World { id: 79, name: "Cactuar".into() },
                World { id: 54, name: "Faerie".into() },
                World { id: 39, name: "Ravana".into() },
            ],
            data_centers: vec![
                DataCenter {
                    name: "Aether".into(),
                    region: "North-America".into(),
                    worlds: vec![79, 54],
                },
                DataCenter {
                    name: "Elemental".into(),
                    region: "Japan".into(),
                    worlds: vec![39],
                },
            ],
        }
    }

    #[test]
    fn a_world_widens_to_its_data_center() {
        assert_eq!(scopes().widen("Cactuar").as_deref(), Some("Aether"));
        assert_eq!(scopes().widen("Ravana").as_deref(), Some("Elemental"));
    }

    #[test]
    fn a_data_center_widens_to_its_region() {
        assert_eq!(scopes().widen("Aether").as_deref(), Some("North-America"));
    }

    #[test]
    fn a_region_has_nowhere_wider_to_go() {
        assert_eq!(scopes().widen("North-America"), None);
    }

    #[test]
    fn widening_tolerates_casing_and_blanks() {
        assert_eq!(scopes().widen(" cactuar ").as_deref(), Some("Aether"));
        assert_eq!(scopes().widen(""), None);
        assert_eq!(scopes().widen("Not A World"), None);
    }


    const SAMPLE: &str = r#"{
        "itemID": 4745,
        "lastUploadTime": 1789049150363,
        "listings": [
            {"pricePerUnit": 900, "quantity": 2, "total": 1800, "hq": true,
             "worldName": "Siren", "retainerName": "Bob"},
            {"pricePerUnit": 100, "quantity": 1, "total": 100, "hq": false,
             "worldName": "Cactuar", "retainerName": ""}
        ],
        "recentHistory": [
            {"pricePerUnit": 120, "quantity": 1, "hq": false, "timestamp": 100,
             "worldName": "Siren"},
            {"pricePerUnit": 130, "quantity": 3, "hq": true, "timestamp": 200,
             "worldName": "Cactuar"}
        ],
        "currentAveragePriceNQ": 256.5,
        "currentAveragePriceHQ": 0,
        "minPriceNQ": 100,
        "minPriceHQ": 0,
        "regularSaleVelocity": 107.28,
        "unitsForSale": 3,
        "unitsSold": 4
    }"#;

    fn sample() -> PriceData {
        let raw: RawMarketData = serde_json::from_str(SAMPLE).unwrap();
        PriceData::from_raw(4745, "Aether", raw)
    }

    #[test]
    fn parses_and_sorts_listings_cheapest_first() {
        let price = sample();
        assert_eq!(
            price
                .listings
                .iter()
                .map(|l| l.price_per_unit)
                .collect::<Vec<_>>(),
            [100, 900]
        );
        assert_eq!(price.listings[0].world_name.as_deref(), Some("Cactuar"));
        assert_eq!(
            price.listings[0].retainer_name, None,
            "empty name is dropped"
        );
        assert_eq!(price.listings[1].retainer_name.as_deref(), Some("Bob"));
    }

    #[test]
    fn sorts_recent_sales_newest_first() {
        let price = sample();
        assert_eq!(
            price
                .recent_sales
                .iter()
                .map(|s| s.timestamp)
                .collect::<Vec<_>>(),
            [200, 100]
        );
    }

    #[test]
    fn treats_zero_prices_as_no_data() {
        let price = sample();
        assert_eq!(price.min_price_nq, Some(100));
        assert_eq!(price.min_price_hq, None);
        assert_eq!(price.average_price_nq, Some(256.5));
        assert_eq!(price.average_price_hq, None);
        assert!(!price.is_empty());
    }

    #[test]
    fn handles_an_item_with_no_market_data() {
        let raw: RawMarketData = serde_json::from_str("{}").unwrap();
        let price = PriceData::from_raw(1, "Cactuar", raw);
        assert!(price.is_empty());
        assert_eq!(price.sale_velocity, 0.0);
        assert_eq!(price.last_upload_time, None);
    }

    #[test]
    fn derives_listing_total_when_absent() {
        let raw: RawMarketData =
            serde_json::from_str(r#"{"listings":[{"pricePerUnit":50,"quantity":4}]}"#).unwrap();
        let price = PriceData::from_raw(1, "Cactuar", raw);
        assert_eq!(price.listings[0].total, 200);
    }

    /// Parses a response captured verbatim from the live API. Guards against
    /// the whole class of bug where a field renames cleanly under serde's
    /// camelCase rule but not on the wire (`minPriceNQ`, not `minPriceNq`) and
    /// every value silently becomes `None`.
    #[test]
    fn parses_a_real_captured_response() {
        const CAPTURED: &str = include_str!("../tests/fixtures/universalis_item.json");
        let raw: RawMarketData = serde_json::from_str(CAPTURED).unwrap();
        let price = PriceData::from_raw(5, "Aether", raw);

        assert!(!price.listings.is_empty(), "fixture has listings");
        assert!(!price.recent_sales.is_empty(), "fixture has sale history");
        assert!(price.min_price_nq.is_some(), "minPriceNQ must deserialise");
        assert!(
            price.average_price_nq.is_some(),
            "currentAveragePriceNQ must deserialise"
        );
        assert!(
            price.sale_velocity > 0.0,
            "regularSaleVelocity must deserialise"
        );
        assert!(price.units_for_sale.is_some());
        assert!(price.last_upload_time.is_some());
        assert!(
            price.listings[0].world_name.is_some(),
            "DC query names each world"
        );
    }

    #[test]
    fn parses_the_multi_item_envelope() {
        let multi: RawMultiMarketData = serde_json::from_str(
            r#"{"itemIDs":[1,2],"items":{"1":{"minPriceNQ":5},"2":{"minPriceNQ":7}}}"#,
        )
        .unwrap();
        assert_eq!(multi.items.len(), 2);
    }

    #[test]
    fn rejects_an_empty_scope() {
        let client = UniversalisClient::new().unwrap();
        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(client.fetch_price(4745, "  "))
            .unwrap_err();
        assert_eq!(err.kind(), "invalid");
    }

    #[test]
    fn urlencodes_scope_names() {
        assert_eq!(urlencode("Light"), "Light");
        assert_eq!(urlencode("North-America"), "North-America");
        assert_eq!(urlencode("a b"), "a%20b");
    }
}
