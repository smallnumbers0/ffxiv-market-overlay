# FFXIV Market Overlay — Build Plan

> Standing architecture, tech stack, and API reference live in `CLAUDE.md`
> and load automatically every session. This file is the task list — work
> through it phase by phase. When asked to "do Phase N," implement only that
> phase, confirm its acceptance criteria are met, then stop.

## Phase 0 — Project scaffolding
- `npm create tauri-app` with React + TypeScript template.
- Confirm `rusqlite`, `reqwest`, `tokio`, `serde`/`serde_json` compile and
  link cleanly on the target OS (Windows first).
- Draft the README skeleton now (sections 1–3, 9, 10 below) — project
  purpose and scope should be explicit from the start.
- **Acceptance:** blank Tauri window opens, hot-reloads on frontend changes.

## Phase 1 — Catalog sync (standalone, no UI)
- Implement `xivapi_sync.rs` as a standalone Rust binary (or a `--sync` CLI
  flag) that populates `items.db` per the schema in `CLAUDE.md`.
- Run it once manually, inspect the resulting SQLite file (row count should
  be in the tens of thousands total; marketable subset much smaller).
- **Acceptance:** `items.db` exists; `marketable = 1` items have correct
  names matching what's shown in-game/on Universalis's own site.

## Phase 2 — Search core
- Implement `search.rs`: load marketable items from SQLite into memory at
  startup, fuzzy-match against the query string.
- Expose `search_items` as a Tauri command.
- Build a bare (non-overlay, normal window) React UI: text input + results list.
- **Acceptance:** typing "hi-potion" or a partial/misspelled name returns
  the correct item within a few keystrokes, no perceptible lag.

## Phase 3 — Universalis client
- Implement `universalis.rs`: request builder for
  `/api/v2/{worldDcRegion}/{itemIds}`, response deserialization into a clean
  internal `PriceData` struct (lowest listings, avg price, velocity).
- Implement `cache.rs` in-memory TTL cache in front of it.
- Expose `get_price` as a Tauri command.
- **Acceptance:** selecting an item in the Phase 2 UI shows live price data
  matching what's visible on universalis.app for the same item/world.

## Phase 4 — Settings & world/DC selection
- `config.rs`: persist chosen world/DC, hotkey, window position to a local
  config file (`tauri-plugin-store` or a simple JSON file in the app data dir).
- Populate a world/DC picker using Universalis's `/api/v2/game/worlds` and
  `/api/v2/game/data-centers`.
- **Acceptance:** setting persists across app restarts; price queries use
  the selected world/DC.

## Phase 5 — Overlay behavior
- Convert the main window to `transparent`, `alwaysOnTop`, `decorations: false`.
- Wire up global hotkey via `tauri-plugin-global-shortcut` to toggle
  show/hide + focus.
- Restore last window position on show.
- **Acceptance:** with FFXIV running in borderless windowed mode, hotkey
  reliably shows/hides the overlay on top of the game without stealing
  persistent focus from the game when hidden.

## Phase 6 — Polish (stretch, not required for a working v1)
- Item icons rendered from XIVAPI asset paths.
- Recent-searches list.
- Click-through mode when overlay is idle/unfocused.
- Multi-item "watchlist" view using Universalis's batch endpoint (up to 100
  IDs/call).
- In-app "Refresh item database" action, reusing Phase 1's sync logic but
  writing to the app-data copy of `items.db`, not the bundled read-only resource.

---

## Required deliverable: README.md

Not optional polish — write and maintain it alongside the code, per the
timing notes below. It needs to work for someone who's never seen the repo.

**Required sections:**

1. **What this is** — plain-language description: search any marketable
   item, see live Universalis prices for your world/DC, without leaving the
   game. State up front it's a standalone companion window — no Dalamud, no
   game-memory reads, no game-file modification — to preempt ToS worries.
2. **Features** — fuzzy item search, live prices (listings/avg/velocity),
   world/DC selection, global-hotkey overlay. Mark anything from Phase 6 as
   "planned" until actually built — never overclaim.
3. **Requirements** — target OS (Windows first), FFXIV in **borderless
   windowed mode** (call out that exclusive fullscreen won't work), WebView2
   prerequisite (note if bundled by the installer).
4. **Installation** (for downloaders, no build tools) — where to get the
   installer (e.g. GitHub Releases), step-by-step install, first-run behavior.
5. **First-time setup** — how to open the overlay (default hotkey, how to
   change it), how to select home world/DC and why it's required first.
6. **Usage** — how to search and read results; how to refresh the item
   catalog after a patch (Phase 6 action, once built).
7. **Building from source** (for contributors) — prerequisites (Rust
   toolchain, Node.js, Tauri CLI, platform Tauri deps); exact commands:
   clone → install deps → run the sync script to generate `resources/items.db`
   → `tauri dev` → `tauri build`. Note `items.db` is generated, not committed,
   so a fresh clone needs the sync step before search has any data.
8. **Troubleshooting** — overlay not appearing over fullscreen FFXIV (switch
   to borderless windowed), "can't reach Universalis" (network/their
   downtime, not an app bug), no search results (catalog not yet synced).
9. **Data sources & credits** — credit Universalis and XIVAPI, link both,
   note this is an unofficial fan tool with no Square Enix affiliation.
10. **License**.

**When to write each part:**
- Phase 0: skeleton (sections 1–3, 9, 10).
- As soon as Phase 0 produces a runnable build: Installation + Building from
  source, verified against a real fresh-clone run, not written speculatively.
- Phases 2–4 land: First-time setup + Usage.
- End of any phase that changes user-facing behavior: update Features +
  Troubleshooting so the README never drifts from what's actually shipped.

---

## Open decisions (agent's discretion)

- Fuzzy-match crate: `nucleo` vs `fuzzy-matcher` vs hand-rolled — pick
  whatever integrates most cleanly with Tauri's async command model.
- `universalis` crate (crates.io) vs hand-rolled `reqwest` client — check the
  crate's last-updated date and endpoint coverage before deciding.
- Config storage: `tauri-plugin-store` vs hand-written JSON — either is
  fine; prefer better first-class Tauri 2.x support at implementation time.
- Default hotkey: `Ctrl+Shift+M` is a reasonable default, but must be
  user-configurable from Phase 4 onward regardless of the initial choice.
