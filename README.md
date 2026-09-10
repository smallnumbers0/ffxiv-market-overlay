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
- **Side-by-side boards** - compare several market boards at once, each its own
  column in the same window. Search once and every board prices the same item
  together, so "is the rest of my data center cheaper?" is a glance rather than
  a second lookup. Adding a column lands on the board one step wider than the
  last, and the cheapest board on screen is flagged. Up to four, remembered
  between launches.
- **HQ and NQ read separately** - the five cheapest HQ listings sit above the
  five cheapest NQ, because they are effectively two different markets for the
  same item and one price-sorted list buries whichever you came to check.
- **Global hotkey overlay** - `Ctrl+Shift+M` by default (`Cmd+Shift+M` on
  macOS), configurable. Shows over the game, hides completely when toggled off.
- **Item icons** - rendered from XIVAPI's asset service.
- **Recent items** - the last dozen items you looked at, one keystroke away.
- **Refresh item database** - re-sync the catalog in-app after a game patch
  adds new items.

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
2. Download **[FFXIV Market Overlay_0.1.0_x64-setup.exe]**.
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

3. **Add a second board**, if you buy and sell across your data center. Press
   **+** on the board strip. The new column arrives already set to your data
   center, so one column tells you what things cost at home and the one beside
   it tells you whether it is worth travelling.

4. **Set FFXIV to borderless windowed mode**, if you have not already. An
   always-on-top window cannot draw over exclusive fullscreen.

---

## Usage

- **Toggle the overlay** with your hotkey, or from the system tray icon.
- **Search** by typing part of an item's name. Matching is fuzzy, so you can
  skip words and letters: `hipot` finds _Hi-Potion_, `savaim x` finds _Savage
  Aim Materia X_, `grade8tinc` finds the Grade 8 Tinctures. Results are ranked
  by how well they match, with earlier and shorter matches first.
- **Move** through results with the up and down arrow keys; **open** the
  highlighted item with `Enter`, or just click it. The search box stays on
  screen while you read prices, so the next item is one query away - type over
  it and the results take the comparison's place until you pick something.
- **Read the prices.** Each board gets a column, left to right, showing:
  - **HQ / NQ** - the lowest current asking price of each quality.
  - **Sales** - sale velocity, in units per day.
  - **Cheapest HQ** and **Cheapest NQ** - the five cheapest listings of each
    quality, with price per unit, quantity, and which world each is on. The HQ
    block is omitted for items that have no HQ version.
  - **cheapest** - flagged on whichever board has the lowest asking price on
    screen. Ties are not flagged: there would be nothing to choose between.
- **Refresh** every board with the circular arrow in the panel header. Prices
  are cached in memory for three minutes, so re-opening an item you just looked
  at is instant; refresh forces a new fetch for every column.
- **`Esc`** steps back one level at a time - a half-typed query first, then the
  open item, then the overlay itself. One key gets you back to the game from
  anywhere.
- **Move the overlay** by dragging its title bar. Its position and size are
  remembered and restored the next time you show it.
- **Add a board** with **+**. It opens on the board one step out from the last
  column - a world opens its data center, a data center opens its region - and
  you can point it anywhere afterwards. Boards are independent: changing one
  never touches another.
- **Re-point a board** by clicking its name in the strip, which opens settings
  for that column.
- **Close a board** with the **x** on its chip. The last one has none: an
  overlay with no board to show is just an empty window. The boards you leave
  open come back the next time you start the app.
- **Three or more boards** scroll sideways rather than squeezing below a
  readable width. Two fit comfortably at the default window size; widen the
  window for more.
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
Try selecting the whole data center instead of a single world - or press **+**
to add a data-center column beside the one you have, and keep both.

**A new column opened on the same board as the last one.**
A new board is set one step out, which needs Universalis's world list. If that
lookup is slow or unavailable, the column opens on the current board instead
rather than making you wait. Click its name in the strip to pick the board you
want; it will be remembered.

**A column shows no world names.**
That board is a single world, so every listing would name the same one -
Universalis does not send a world on a single-world query. Point the column at
a data center or region and the world each listing sits on appears, which is
the whole point of a wide column: it tells you where to travel.

**The listings show no total.**
Side-by-side columns are narrow, and total is price times quantity - both of
which are already on the row.

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
