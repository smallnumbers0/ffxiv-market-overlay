import { useCallback, useEffect, useRef, useState } from "react";

import { ComparisonPanel, type BoardPrice } from "./components/ComparisonPanel";
import { ResultsList } from "./components/ResultsList";
import { SearchBox } from "./components/SearchBox";
import { SettingsPanel } from "./components/SettingsPanel";
import { BoardStrip } from "./components/BoardStrip";
import { TitleBar } from "./components/TitleBar";
import { UpdateBanner } from "./components/UpdateBanner";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { useUpdate } from "./hooks/useUpdate";
import { guessRegion } from "./lib/region";
import {
  addBoard,
  ensureMarketScope,
  getPrice,
  getRecentItems,
  getSettings,
  recordRecentItem,
  removeBoard,
  hideOverlay,
  searchItems,
  toAppError,
  type AppError,
  type Board,
  type Item,
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
  const update = useUpdate();

  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, SEARCH_DEBOUNCE_MS);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [recents, setRecents] = useState<Item[]>([]);
  const [highlightIndex, setHighlightIndex] = useState(0);

  const [selected, setSelected] = useState<Item | null>(null);
  /** Prices for the open item, keyed by board id - one per column. */
  const [prices, setPrices] = useState<Record<string, BoardPrice>>({});
  /** Which board the settings panel is editing. */
  const [editingBoard, setEditingBoard] = useState<string | null>(null);

  const boards: Board[] = settings?.boards ?? [];
  /** The board the settings panel edits - the first one unless a chip was clicked. */
  const editing =
    boards.find((board) => board.id === editingBoard) ?? boards[0] ?? null;
  /** Setup is done once every column has a board to query. */
  const ready = boards.length > 0 && boards.every((board) => board.scope);

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
      const wasIncomplete = settings?.boards.some((board) => !board.scope);
      if (wasIncomplete && next.boards.every((board) => board.scope)) {
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
    const blank = settings.boards.find((board) => !board.scope);
    if (blank) {
      ensureMarketScope(blank.id, guessRegion())
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

  /**
   * Price `itemId` on every board at once. Columns fill in as their requests
   * land rather than waiting for the slowest board, so a data centre that is
   * slow to answer never holds up the world beside it.
   */
  const loadPrices = useCallback(
    (list: Board[], itemId: number, refresh = false) => {
      const token = ++priceToken.current;
      setPrices(
        Object.fromEntries(
          list.map((board) => [
            board.id,
            { price: null, loading: true, error: null },
          ]),
        ),
      );

      for (const board of list) {
        if (!board.scope) continue;
        getPrice(board.id, itemId, refresh)
          .then((price) => {
            if (token !== priceToken.current) return;
            setPrices((current) => ({
              ...current,
              [board.id]: { price, loading: false, error: null },
            }));
          })
          .catch((cause) => {
            if (token !== priceToken.current) return;
            setPrices((current) => ({
              ...current,
              [board.id]: {
                price: null,
                loading: false,
                error: toAppError(cause),
              },
            }));
          });
      }
    },
    [],
  );

  // Re-price when the item changes, and when the set of boards does - adding a
  // column or re-pointing one has to fill it in without a fresh search.
  const boardKey = boards.map((board) => `${board.id}:${board.scope}`).join(",");
  useEffect(() => {
    if (!selected || boards.length === 0) return;
    loadPrices(boards, selected.itemId);
    // `boardKey` stands in for `boards`, which is a new array every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, boardKey, loadPrices]);

  const handleAddBoard = useCallback(() => {
    addBoard()
      .then(setSettings)
      .catch((cause) => setSettingsError(toAppError(cause)));
  }, []);

  const handleRemoveBoard = useCallback((id: string) => {
    removeBoard(id)
      .then(setSettings)
      .catch((cause) => setSettingsError(toAppError(cause)));
  }, []);

  const handleEditBoard = useCallback((id: string) => {
    setEditingBoard(id);
    setShowSettings(true);
  }, []);

  const selectItem = useCallback((item: Item) => {
    setSelected(item);
    setPrices({});
    // Hand the space back to the comparison. The search box stays put and
    // keeps focus, so the next item is one query away rather than one
    // dismissal and one query away.
    setQuery("");
    // Opening an item is what makes it "recent" - prefetching one the user
    // only highlighted must not.
    void recordRecentItem(item.itemId).then(() => getRecentItems().then(setRecents));
  }, []);

  const visibleItems: Item[] = query.trim() ? results : recents;
  /**
   * Whether a list of items is on screen. The search box is always there, but
   * below it sits either a list or the comparison, never both - at overlay
   * height there is not room for two.
   */
  const listVisible = Boolean(query.trim()) || !selected;

  // Warm the cache for whatever is highlighted, so selecting it is instant.
  // `get_price` is cache-first, so this costs one request per item per TTL.
  useEffect(() => {
    if (showSettings || !ready || !listVisible) return;
    const candidate = visibleItems[highlightIndex];
    if (!candidate) return;
    const timer = setTimeout(() => {
      // Every column, since opening the item shows them all.
      for (const board of boards) {
        void getPrice(board.id, candidate.itemId).catch(() => {
          /* A failed prefetch is invisible; opening it surfaces the error. */
        });
      }
    }, PREFETCH_DELAY_MS);
    return () => clearTimeout(timer);
  }, [visibleItems, highlightIndex, listVisible, showSettings, ready, boardKey]);

  // --- Keyboard -------------------------------------------------------------

  const closePanel = useCallback(() => {
    setSelected(null);
    setPrices({});
    searchInput.current?.focus();
  }, []);

  const handleSearchKeys = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (!listVisible) return;
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
  // single key gets you from anywhere back to the game. A half-typed query
  // goes first: it is the thing covering the comparison.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      if (query) setQuery("");
      else if (showSettings && ready) setShowSettings(false);
      else if (selected) closePanel();
      else void hideOverlay();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selected, showSettings, query, ready, closePanel]);

  // --- Render ---------------------------------------------------------------

  const catalogReady = settings?.catalogReady ?? false;

  return (
    <div className="app">
      <TitleBar
        showSettings={showSettings}
        onToggleSettings={() => setShowSettings((open) => !open)}
      />

      {settings && (
        <BoardStrip
          boards={settings.boards}
          canAddBoard={settings.canAddBoard}
          editing={showSettings ? (editing?.id ?? null) : null}
          onEdit={handleEditBoard}
          onRemove={handleRemoveBoard}
          onAdd={handleAddBoard}
        />
      )}

      <main className="content">
        <UpdateBanner
          stage={update.stage}
          onInstall={() => void update.install()}
          onDismiss={update.dismiss}
        />

        {settingsError && (
          <p className="error-message" role="alert">
            {settingsError.message}
          </p>
        )}

        {showSettings && settings && editing ? (
          <SettingsPanel
            settings={settings}
            board={editing}
            onSettingsChange={applySettings}
            onCatalogRefreshed={() => void reloadSettings()}
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

            {boards.some((board) => board.scopeIsGuess) && (
              <button
                type="button"
                className="scope-nudge"
                onClick={() =>
                  handleEditBoard(
                    boards.find((board) => board.scopeIsGuess)!.id,
                  )
                }
              >
                Showing{" "}
                <strong>
                  {boards.find((board) => board.scopeIsGuess)!.scope}
                </strong>{" "}
                prices, guessed from your time zone. Pick your home world for
                prices you can actually buy at.
              </button>
            )}

            {query.trim() ? (
              <ResultsList
                results={results}
                highlightIndex={highlightIndex}
                onHighlight={setHighlightIndex}
                onSelect={selectItem}
                emptyMessage={`No marketable item matches "${query.trim()}".`}
              />
            ) : selected ? (
              <ComparisonPanel
                item={selected}
                boards={boards}
                prices={prices}
                onRefresh={() => loadPrices(boards, selected.itemId, true)}
              />
            ) : (
              <>
                {recents.length > 0 && <p className="list-label">Recent</p>}
                <ResultsList
                  results={recents}
                  highlightIndex={highlightIndex}
                  onHighlight={setHighlightIndex}
                  onSelect={selectItem}
                  emptyMessage="Type to search marketable items."
                />
              </>
            )}
          </>
        )}
      </main>
    </div>
  );
}
