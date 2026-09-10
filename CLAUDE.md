# FFXIV Market Overlay

## What this is

A desktop overlay for Final Fantasy XIV: search any marketable item by name,
see live prices from Universalis for the player's home world/data center,
without alt-tabbing or traveling to a market board. Toggled by a global
hotkey, shown as a transparent always-on-top window over the game
(borderless windowed mode, not exclusive fullscreen).

Standalone companion window — **not** a Dalamud plugin, does not read FFXIV's
process memory, does not inject into the game, does not modify game files.
Don't add anything that would change this.

Distributed as an installable app run by many end users. There is no shared
backend server. See "Architecture principle" below.

For the phased build order and acceptance criteria, see `PLAN.md`. Work one
phase at a time unless told otherwise — don't jump ahead to later phases.

## Tech stack

| Layer | Choice | Notes |
|---|---|---|
| App shell | Tauri 2.x | Rust backend, native OS webview (WebView2 on Windows) |
| Frontend | React + TypeScript | Inside Tauri's webview |
| Fuzzy search | Rust-side (`nucleo` or `fuzzy-matcher`) | Keep in Rust — don't marshal the whole item table across the JS bridge per keystroke |
| Local storage | SQLite via `rusqlite` | Bundled, no external SQLite install needed |
| HTTP client | `reqwest` + `tokio` | For both XIVAPI sync and live Universalis calls |
| Global hotkey | `tauri-plugin-global-shortcut` | Toggle overlay visibility |
| Window config | `transparent: true`, `alwaysOnTop: true`, `decorations: false`, `skipTaskbar: true` | Click-through via `set_ignore_cursor_events` is a later-phase nice-to-have, not v1 |

No API keys or authentication anywhere in this project. Don't add auth
scaffolding that isn't needed.

## Architecture principle: local SQLite, no shared backend

Two data types, opposite lifecycles:

- **Item ID ↔ name mapping** — static, identical for every player, changes
  only on game patches. Baked into the app, shipped to every user's machine
  as a bundled SQLite file.
- **Live prices** — genuinely shared and constantly changing. Universalis is
  already the centralized source of truth. Call it directly per user; don't
  proxy or centrally cache prices.

No custom backend service for v1. Each installed copy is self-sufficient
once its item catalog is synced.

## External APIs

### XIVAPI v2 (static item data)
- Base: `https://v2.xivapi.com`
- **Never use `xivapi.com` (v1) or `/docs/Search`** — deprecated, different
  schema/search syntax entirely.
- Bulk sync: `GET /api/sheet/Item?fields=Name,Icon,LevelItem,ItemUICategory.Name&after={lastRowId}&limit={n}`
  — paginate via `after` = last `row_id` seen; stop when a page returns fewer
  rows than `limit`. Row IDs are **not contiguous**.
- Single item: `GET /api/sheet/Item/{rowId}?fields=...`
- No auth required. Don't use `/api/search` — we fuzzy-search the local
  synced catalog instead of hitting XIVAPI per keystroke.

### Universalis (live price data)
- Base: `https://universalis.app/api/v2`
- Current prices: `GET /api/v2/{worldDcRegion}/{itemIds}` — world name, DC
  name, or region name; up to 100 comma-separated item IDs per request.
  Returns active listings, sale history, average price, sale velocity.
- Sale history: `GET /api/v2/history/{worldDcRegion}/{itemIds}`
- `GET /api/v2/worlds`, `GET /api/v2/data-centers` — for the
  settings picker.
- `GET /api/v2/marketable` — item IDs that are actually tradable; use
  this to filter the local catalog so search never surfaces non-tradable items.
- No auth required. Rate limit: 25 req/s (50 burst), 8 concurrent
  connections/IP — irrelevant for single-user desktop use, but still cache
  (see below).
- A `universalis` crate exists on crates.io; check its maintenance status
  before adopting vs. hand-rolling with `reqwest`.

## Data model (`items.db`, bundled SQLite)

```sql
CREATE TABLE items (
    item_id       INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    icon_path     TEXT,
    level_item    INTEGER,
    category_name TEXT,
    marketable    INTEGER NOT NULL DEFAULT 0  -- 1 if in Universalis /marketable
);
CREATE INDEX idx_items_name ON items(name);

CREATE TABLE sync_meta (
    key   TEXT PRIMARY KEY,
    value TEXT
);
-- rows: schema_version, last_synced_game_version, last_synced_at
```

Only `marketable = 1` rows should ever surface in search results.
`resources/items.db` is a **generated build artifact** — regenerate via the
sync script whenever schema/fields change, never hand-edit it.

## Live price flow

1. Debounce search input (~150ms) → `search_items(query)` Tauri command →
   fuzzy match against the in-memory-resident marketable item set. No
   network call.
2. On item selection → `get_price(item_id, world_or_dc)` → check in-memory
   `HashMap<(item_id, world), (Instant, PriceData)>` cache (TTL 2–5 min) →
   on miss, call Universalis, cache result.
3. Cache is per-session (in-memory, cleared on restart) — no persistent
   price cache needed in v1.

## Project structure

```
ffxiv-market-overlay/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands.rs      # #[tauri::command] functions
│   │   ├── db.rs             # rusqlite setup/queries
│   │   ├── search.rs          # fuzzy matching
│   │   ├── universalis.rs     # Universalis HTTP client + types
│   │   ├── xivapi_sync.rs     # catalog sync logic
│   │   ├── cache.rs            # in-memory price cache
│   │   └── config.rs            # settings: world/DC, hotkey, window pos
│   ├── resources/items.db      # generated, bundled — not hand-written
│   └── tauri.conf.json
├── src/                        # React frontend
│   ├── components/{SearchBox,ResultsList,PricePanel,SettingsPanel}.tsx
│   └── lib/tauriApi.ts         # typed invoke() wrappers
├── src-tauri/src/bin/sync_catalog.rs  # build-time catalog generator
│                               # (lives in the crate so it shares db.rs +
│                               #  xivapi_sync.rs; run via `npm run sync-catalog`)
└── PLAN.md
```

## Non-functional rules

- All Universalis/XIVAPI calls: reasonable timeout (~5s), surface a clear
  "couldn't reach Universalis" UI state on failure rather than hanging —
  this gets checked mid-gameplay, failures must be visible and fast.
- Keep the fuzzy-match dataset resident in memory; SQLite is the
  startup-load source of truth, not the per-keystroke hot path.
- README.md is a required deliverable, written alongside the code — see
  PLAN.md for its required sections and when to fill each one in.
