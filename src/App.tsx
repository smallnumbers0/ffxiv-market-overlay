import { useCallback, useEffect, useRef, useState } from "react";

import { PricePanel } from "./components/PricePanel";
import { ResultsList } from "./components/ResultsList";
import { SearchBox } from "./components/SearchBox";
import { SettingsPanel } from "./components/SettingsPanel";
import { TitleBar } from "./components/TitleBar";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { guessRegion } from "./lib/region";
import {
  ensureMarketScope,
  getPrice,
  getRecentItems,
  getSettings,
  recordRecentItem,
  hideOverlay,
  searchItems,
  toAppError,
  type AppError,
  type Item,
  type PriceData,
  type SearchResult,
  type Settings,
} from "./lib/tauriApi";

/**
 * Search is local (Rust, in-memory), so this only coalesces a fast typist's
 * keystrokes into one scoring pass. A network-backed search would want the
 * ~150ms a human reads as "instant"; here that delay would be pure added lag.
 */
const SEARCH_DEBOUNCE_MS = 40;

/**
 * How long an item must stay highlighted before its price is fetched ahead of
 * a click. Long enough that arrowing through a list, or sweeping the mouse
 * across it, doesn't fire a request per row; short enough that the price is
 * already cached by the time a user who paused to read the row clicks it.
 */
const PREFETCH_DELAY_MS = 300;

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsError, setSettingsError] = useState<AppError | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, SEARCH_DEBOUNCE_MS);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [recents, setRecents] = useState<Item[]>([]);
  const [highlightIndex, setHighlightIndex] = useState(0);

  const [selected, setSelected] = useState<Item | null>(null);
  const [price, setPrice] = useState<PriceData | null>(null);
  const [priceLoading, setPriceLoading] = useState(false);
  const [priceError, setPriceError] = useState<AppError | null>(null);

  const searchInput = useRef<HTMLInputElement>(null);
  /** Guards against an older search resolving after a newer one. */
  const searchToken = useRef(0);
  /** Same, for price lookups. */
  const priceToken = useRef(0);

  const reloadSettings = useCallback(async () => {
    try {
      const next = await getSettings();
      setSettings(next);
      setSettingsError(null);
      setRecents(await getRecentItems());
    } catch (cause) {
      setSettingsError(toAppError(cause));
    }
  }, []);

  useEffect(() => {
    void reloadSettings();
  }, [reloadSettings]);

  /**
   * Settings opens itself when setup is incomplete; close it again the moment
   * the last missing piece arrives, so first-run lands the user in the search
   * box rather than on a panel they now have no reason to be looking at.
   */
  const applySettings = useCallback(
    (next: Settings) => {
      if (settings && !settings.marketScope && next.marketScope) {
        setShowSettings(false);
      }
      setSettings(next);
    },
    [settings],
  );

  /**
   * First launch: seed a region from the system time zone so the app is
   * usable immediately, and only fall back to the settings panel when there
   * is genuinely nothing to search (no catalog at all).
   */
  useEffect(() => {
    if (!settings) return;
    if (!settings.marketScope) {
      ensureMarketScope(guessRegion())
        .then(setSettings)
        .catch((cause) => setSettingsError(toAppError(cause)));
      return;
    }
    if (!settings.catalogReady) setShowSettings(true);
  }, [settings]);

  // --- Search ---------------------------------------------------------------

  useEffect(() => {
    const token = ++searchToken.current;
    const trimmed = debouncedQuery.trim();
    if (!trimmed) {
      setResults([]);
      setHighlightIndex(0);
      return;
    }
    searchItems(trimmed)
      .then((found) => {
        if (token !== searchToken.current) return;
        setResults(found);
        setHighlightIndex(0);
      })
      .catch((cause) => {
        if (token === searchToken.current) setSettingsError(toAppError(cause));
      });
  }, [debouncedQuery]);

  // --- Prices ---------------------------------------------------------------

  const loadPrice = useCallback(async (itemId: number, refresh = false) => {
    const token = ++priceToken.current;
    setPriceLoading(true);
    setPriceError(null);
    try {
      const data = await getPrice(itemId, refresh);
      if (token === priceToken.current) setPrice(data);
    } catch (cause) {
      if (token === priceToken.current) {
        setPriceError(toAppError(cause));
        setPrice(null);
      }
    } finally {
      if (token === priceToken.current) setPriceLoading(false);
    }
  }, []);

  const selectItem = useCallback(
    (item: Item) => {
      setSelected(item);
      setPrice(null);
      // Opening an item is what makes it "recent" - prefetching one the user
      // only highlighted must not.
      void recordRecentItem(item.itemId).then(() => void reloadSettings());
      void loadPrice(item.itemId);
    },
    [loadPrice, reloadSettings],
  );

  const visibleItems: Item[] = query.trim() ? results : recents;

  // Warm the cache for whatever is highlighted, so selecting it is instant.
  // `get_price` is cache-first, so this costs one request per item per TTL.
  useEffect(() => {
    if (selected || showSettings || !settings?.marketScope) return;
    const candidate = visibleItems[highlightIndex];
    if (!candidate) return;
    const timer = setTimeout(() => {
      void getPrice(candidate.itemId).catch(() => {
        /* A failed prefetch is invisible; selecting it will surface the error. */
      });
    }, PREFETCH_DELAY_MS);
    return () => clearTimeout(timer);
  }, [visibleItems, highlightIndex, selected, showSettings, settings?.marketScope]);

  // --- Keyboard -------------------------------------------------------------

  const closePanel = useCallback(() => {
    setSelected(null);
    setPrice(null);
    setPriceError(null);
    searchInput.current?.focus();
  }, []);

  const handleSearchKeys = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (visibleItems.length === 0) return;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setHighlightIndex(
        (index) =>
          (index + step + visibleItems.length) % visibleItems.length,
      );
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const item = visibleItems[highlightIndex];
      if (item) selectItem(item);
    }
  };

  // Escape backs out one level at a time, then hides the overlay - so a
  // single key gets you from anywhere back to the game.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      if (selected) closePanel();
      else if (showSettings && settings?.marketScope) setShowSettings(false);
      else if (query) setQuery("");
      else void hideOverlay();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selected, showSettings, query, settings?.marketScope, closePanel]);

  // --- Render ---------------------------------------------------------------

  const catalogReady = settings?.catalogReady ?? false;

  return (
    <div className="app">
      <TitleBar
        scope={settings?.marketScope ?? null}
        showSettings={showSettings}
        onToggleSettings={() => setShowSettings((open) => !open)}
      />

      <main className="content">
        {settingsError && (
          <p className="error-message" role="alert">
            {settingsError.message}
          </p>
        )}

        {showSettings && settings ? (
          <SettingsPanel
            settings={settings}
            onSettingsChange={applySettings}
            onCatalogRefreshed={() => void reloadSettings()}
          />
        ) : selected ? (
          <PricePanel
            item={selected}
            price={price}
            loading={priceLoading}
            error={priceError}
            onBack={closePanel}
            onRefresh={() => void loadPrice(selected.itemId, true)}
          />
        ) : (
          <>
            <SearchBox
              ref={searchInput}
              value={query}
              onChange={setQuery}
              onKeyDown={handleSearchKeys}
              disabled={!catalogReady}
              placeholder={
                catalogReady ? "Search items..." : "No item catalog yet"
              }
            />
            {settings?.marketScopeIsGuess && (
              <button
                type="button"
                className="scope-nudge"
                onClick={() => setShowSettings(true)}
              >
                Showing <strong>{settings.marketScope}</strong> prices, guessed
                from your time zone. Pick your home world for prices you can
                actually buy at.
              </button>
            )}
            {!query.trim() && recents.length > 0 && (
              <p className="list-label">Recent</p>
            )}
            <ResultsList
              results={visibleItems}
              highlightIndex={highlightIndex}
              onHighlight={setHighlightIndex}
              onSelect={selectItem}
              emptyMessage={
                query.trim()
                  ? `No marketable item matches "${query.trim()}".`
                  : "Type to search marketable items."
              }
            />
          </>
        )}
      </main>
    </div>
  );
}
