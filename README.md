# FFXIV Market Overlay

Ever find it time consuming to travel to the market board in game? Tired of opening universalis on another monitor?

Search any Final Fantasy XIV item and see live market board prices
for your home world or data center and compare without the tedious time consuming efforts.

**This is a standalone companion window, not a game modification.**

It does not read Final Fantasy XIV's process memory, inject anything into the
game, or modify any game file.
It is an ordinary desktop application that talks to two public websites over
HTTPS - [XIVAPI](https://v2.xivapi.com) for item names and
[Universalis](https://universalis.app) for prices - and draws itself on top of
your game window.

---

## Features

- **Fuzzy item search** - type `hipot` for _Hi-Potion_, `darkmat` for _Dark
  Matter Cluster_, `grade8tinc` for the Grade 8 Tinctures. Search runs entirely
  on your machine against a local catalog, so it responds in single-digit
  milliseconds with no network round trip.
- **Only marketable items** - the catalog is filtered by Universalis's own
  tradability list, so search never surfaces an item you cannot buy or sell.
- **Live prices** - cheapest NQ and HQ listings, current average prices, sale
  velocity, units for sale, the cheapest individual listings, and recent sale
  history.
- **World or data center** - query a single world, a whole data center, or an
  entire region. Defaults to your region on first launch so the app is useful
  before you configure anything.
- **Global hotkey overlay** - `Ctrl+Shift+M` by default (`Cmd+Shift+M` on
  macOS), configurable. Shows over the game, hides completely when toggled off.
- **Item icons** - rendered from XIVAPI's asset service.
- **Recent items** - the last dozen items you looked at, one keystroke away.
- **Refresh item database** - re-sync the catalog in-app after a game patch
  adds new items.

Planned, not built yet:

- Click-through mode when the overlay is idle.
- A multi-item watchlist view. (`UniversalisClient::fetch_prices` already
  implements the batched call it needs; the view itself is not built.)

---

## Requirements

- **Windows 10 or 11.** Windows is the primary target. The app also builds and
  runs on macOS and Linux, which is useful for development, but the overlay
  behaviour is tuned for Windows.
- **Final Fantasy XIV in borderless windowed mode.** An always-on-top window
  cannot draw over a game running in _exclusive fullscreen_ - this is an
  operating system limitation, not something the app can work around. Set
  **System Configuration → Graphics Settings → Screen Mode → Borderless
  Windowed** in game.
- **WebView2 runtime.** Already present on up-to-date Windows 10 and 11
  installs, and embedded in the installer for the rare machine that lacks it.
  Nothing for you to do either way.
- **An internet connection** for price lookups. Item search works offline once
  the catalog is installed.

No account, no API key, and no login - neither XIVAPI nor Universalis requires
authentication, and this app has no server of its own.

---

## Installation

For people who just want to run it. No build tools, no configuration, no
account.

1. Go to the project's **[Releases]** page on GitHub.
2. Download `FFXIV.Market.Overlay_x.y.z_x64-setup.exe`.
3. Run it.

   Windows will show **"Windows protected your PC"**, because the installer is
   not code-signed. Click **More info**, then **Run anyway**. This is normal
   for community FFXIV tools and is a statement about the certificate, not
   about the file.

4. Launch **FFXIV Market Overlay** from the Start menu.

That is the whole installation. Specifically:

- **No admin prompt.** It installs for your user account only.
- **No separate runtime download.** The WebView2 bootstrapper is embedded, and
  on current Windows 10/11 WebView2 is already present anyway.
- **No item database to sync.** The full catalog of ~50,000 items ships inside
  the installer and unpacks itself the first time you open the app.
- **No world to configure before it works.** The app picks your region from
  your system time zone and shows real prices immediately. Narrowing that to
  your own world is a one-click improvement, not a prerequisite - see below.

[Releases]: ../../releases

The app keeps its files in:

- Settings: `%APPDATA%\com.ffxivmarketoverlay.desktop\config.json`
- Item catalog: `%APPDATA%\com.ffxivmarketoverlay.desktop\items.db`

Deleting those two files resets the app completely. Uninstall from
**Settings → Apps → Installed apps** like any other program.

---

## Getting the most out of it

None of this is required - the app works the moment it opens - but two minutes
here makes it substantially more useful.

1. **Set your home world.** On first launch the app shows prices for your whole
   region, guessed from your system time zone, and says so in a banner. Region
   prices tell you what an item is worth; they do not tell you what you can
   actually buy without travelling. Open settings (the sliders icon in the
   title bar) and pick your world - or your data center, if you are willing to
   travel for a good price. The banner disappears once you choose.

2. **Check the hotkey.** The default is `Ctrl+Shift+M`. To change it, type a
   new accelerator into the hotkey field and press **Apply** - for example
   `Alt+M`, `F9`, or `CmdOrControl+Shift+P`. If another application already
   owns that combination, the app says so and keeps the previous hotkey rather
   than leaving you with none.

3. **Set FFXIV to borderless windowed mode**, if you have not already. An
   always-on-top window cannot draw over exclusive fullscreen.

---

## Usage

- **Toggle the overlay** with your hotkey, or from the system tray icon.
- **Search** by typing part of an item's name. Matching is fuzzy, so you can
  skip words and letters: `hipot` finds _Hi-Potion_, `savaim x` finds _Savage
  Aim Materia X_, `grade8tinc` finds the Grade 8 Tinctures. Results are ranked
  by how well they match, with earlier and shorter matches first.
- **Move** through results with the up and down arrow keys; **open** the
  highlighted item with `Enter`, or just click it.
- **Read the prices.** The panel shows, for your selected world, data center,
  or region:
  - **Cheapest NQ / HQ** - the lowest current asking price per unit.
  - **Avg NQ / HQ** - Universalis's current average price.
  - **Sales** - sale velocity, in units per day.
  - **For sale** - how many units are listed.
  - **Cheapest listings** - individual listings, price per unit, quantity,
    total, and which world each is on.
  - **Recent sales** - what the item actually sold for, and when.
  - The footer shows how long ago Universalis last received an upload for that
    item, and how fresh the numbers on screen are.
- **Refresh** a single item's prices with the circular arrow in the panel
  header. Prices are cached in memory for three minutes, so re-opening an item
  you just looked at is instant; refresh forces a new fetch.
- **`Esc`** steps back one level at a time - price panel to results, results to
  an empty box, empty box to hidden. One key gets you back to the game from
  anywhere.
- **Move the overlay** by dragging its title bar. Its position and size are
  remembered and restored the next time you show it.
- **Quit** from the system tray icon. Closing the window only hides it, so the
  hotkey keeps working.
- **If the hotkey doesn't register** (another app already owns that
  combination), the settings panel says so and the tray icon still opens the
  overlay - you are never locked out.

### After a game patch

New items added by a patch will not appear in search until the catalog is
re-synced. Open settings and press **Refresh item database**. It takes about a
minute, downloads roughly 50,000 item names, and replaces the catalog in a
single transaction - if it fails partway, your existing catalog is untouched.

You can also just install the next release, which ships a catalog built at
release time.

---

## Building from source

### Prerequisites

- **Rust** (stable) - <https://rustup.rs>
- **Node.js** 18 or newer, with npm
- **Platform build tools for Tauri 2** - see
  <https://tauri.app/start/prerequisites/>:
  - Windows: Microsoft C++ Build Tools and the WebView2 runtime
  - macOS: Xcode Command Line Tools
  - Linux: `webkit2gtk`, `libayatana-appindicator`, and friends

The Tauri CLI comes from `package.json`; there is nothing to install globally.

### Build and run

```bash
git clone <repository-url>
cd ffxiv-market-overlay
npm install

# Generate the item catalog. Required on a fresh clone: items.db is a build
# artifact and is not committed, so search has no data until this runs.
# Takes about a minute and writes src-tauri/resources/items.db.
npm run sync-catalog

npm run tauri dev      # development, with frontend hot reload
npm run tauri build    # installers in src-tauri/target/release/bundle/
```

`npm run sync-catalog` accepts an optional output path:
`npm run sync-catalog -- /tmp/items.db`.

A clone will compile without running the sync first - the build script drops an
empty placeholder catalog in so the crate builds - but the app will open on its
settings panel and tell you to sync, because an empty catalog is not a usable
one.

### Tests

```bash
npm test                      # frontend logic (vitest)
npx tsc --noEmit              # TypeScript type check
cd src-tauri && cargo test    # backend
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

CI runs all of these on Linux and Windows for every push and pull request.

The Rust tests cover catalog storage, fuzzy ranking, price parsing (including
against a response captured verbatim from the live Universalis API), the TTL
cache, settings persistence, and app state. They do not hit the network. The
frontend tests cover price/time formatting and the time-zone-to-region mapping
that drives the first-launch default.

`src-tauri/tests/real_catalog.rs` additionally checks ranking and search speed
against the real generated catalog; those tests skip themselves if you haven't
run `npm run sync-catalog`.

### Layout

```
src-tauri/src/
  lib.rs           app wiring: window, tray, hotkey, catalog bootstrap
  commands.rs      every #[tauri::command] the frontend can call
  state.rs         shared state handed to those commands
  db.rs            SQLite schema, load, atomic bulk replace
  search.rs        in-memory fuzzy matching (nucleo)
  universalis.rs   Universalis HTTP client and price types
  xivapi_sync.rs   catalog sync logic
  cache.rs         in-memory TTL price cache
  config.rs        settings persistence
  hotkey.rs        global shortcut and show/hide
  bin/sync_catalog.rs   the build-time catalog generator
src/
  App.tsx          screen orchestration and keyboard handling
  components/      SearchBox, ResultsList, PricePanel, SettingsPanel, TitleBar
  lib/tauriApi.ts  typed invoke() wrappers - the only file that calls invoke
  lib/format.ts    display formatting
  lib/region.ts    time zone -> Universalis region, for the first-launch default
```

### Releasing

Installers are built by GitHub Actions, not by hand - `.github/workflows/release.yml`
runs on a `windows-latest` runner, so the Windows binary is built on Windows.

```bash
npm version patch      # or minor / major - writes package.json and tags
git push --follow-tags
```

The workflow type-checks, runs both test suites, generates a fresh `items.db`,
refuses to continue if that catalog looks truncated, builds the NSIS installer,
and attaches it to a **draft** GitHub Release for you to review and publish.
Run the workflow manually from the Actions tab to get an installer as a
workflow artifact without publishing anything.

Bump the version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` to
match before tagging.

**Code signing** is not set up: releases are unsigned, so users see a
SmartScreen warning once. This is the norm for community FFXIV tools. To
change that, add a certificate and set `bundle.windows.certificateThumbprint`,
`digestAlgorithm`, and `timestampUrl` in `tauri.conf.json`, or point
`bundle.windows.signCommand` at a signing service - the rest of the pipeline
needs no changes.

---

## Troubleshooting

**The overlay does not appear over the game.**
Final Fantasy XIV is almost certainly in exclusive fullscreen. Switch to
**System Configuration → Graphics Settings → Screen Mode → Borderless
Windowed**. No always-on-top window can draw over an exclusive-fullscreen
application.

**"Windows protected your PC" when running the installer.**
Expected. The installer is not code-signed. Click **More info**, then **Run
anyway**. Antivirus tools occasionally flag unsigned installers for the same
reason; if yours quarantines it, the file is on GitHub Releases and you can
check its hash there.

**Prices look wrong, or nothing is buyable.**
Check the world name in the title bar. Until you pick one, the app shows prices
for your whole region, which includes worlds you would have to travel to. Open
settings and choose your home world.

**The hotkey does nothing.**
Another application has probably already claimed that combination. Open the
overlay from the system tray icon, then set a different hotkey in settings.

**"Couldn't reach Universalis".**
Universalis is a third-party community service and is occasionally down or slow.
This is not a fault in the app and there is nothing to fix locally - press
**Try again**, or check <https://universalis.app> in a browser. Item search
keeps working while prices are unavailable, because the catalog is local.

**Search returns nothing at all.**
The item catalog has not been synced. Open settings and press **Refresh item
database**. On a build from source, run `npm run sync-catalog` first.

**An item exists in game but not in search.**
Either it was added in a patch newer than your catalog (refresh the item
database), or it is not tradable on the market board - untradable items are
deliberately excluded.

**A price says "no data".**
Universalis is crowd-sourced: it only knows what players' market board visits
have uploaded. Rarely traded items on quiet worlds genuinely have no data.
Try selecting the whole data center instead of a single world.

---

## Data sources and credits

- **[Universalis](https://universalis.app)** - crowd-sourced market board
  prices. Every price in this app comes from Universalis's public API, called
  directly from your machine. Please consider contributing uploads back to
  them.
- **[XIVAPI](https://xivapi.com)** (v2) - item names, icons, item levels, and
  categories.

This is an unofficial fan-made tool. It is not affiliated with, endorsed by, or
associated with Square Enix.

FINAL FANTASY XIV © SQUARE ENIX CO., LTD. All game content and materials are
trademarks and copyrights of Square Enix.

---

## License

MIT. See [LICENSE](LICENSE).
