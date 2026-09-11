/**
 * Typed wrappers around every Tauri command.
 *
 * The types here mirror the `Serialize` shapes in `src-tauri/src`. Keeping
 * every `invoke()` call behind this module means a Rust signature change
 * breaks the build in one place instead of silently returning `undefined`
 * somewhere deep in a component.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// --- Types (mirror of the Rust structs) -------------------------------------

export interface Item {
  itemId: number;
  name: string;
  iconPath: string | null;
  levelItem: number | null;
  categoryName: string | null;
}

export interface SearchResult extends Item {
  score: number;
}

export interface Listing {
  pricePerUnit: number;
  quantity: number;
  total: number;
  hq: boolean;
  worldName: string | null;
  retainerName: string | null;
}

export interface Sale {
  pricePerUnit: number;
  quantity: number;
  hq: boolean;
  /** Unix seconds. */
  timestamp: number;
  worldName: string | null;
}

export interface PriceData {
  itemId: number;
  scope: string;
  listings: Listing[];
  recentSales: Sale[];
  minPriceNq: number | null;
  minPriceHq: number | null;
  averagePriceNq: number | null;
  averagePriceHq: number | null;
  saleVelocity: number;
  unitsForSale: number | null;
  unitsSold: number | null;
  /** Unix milliseconds. */
  lastUploadTime: number | null;
  /** Unix milliseconds. */
  fetchedAt: number;
}

export interface World {
  id: number;
  name: string;
}

export interface DataCenter {
  name: string;
  region: string;
  worlds: number[];
}

export interface MarketScopes {
  worlds: World[];
  dataCenters: DataCenter[];
}

export interface CatalogInfo {
  totalItems: number;
  marketableItems: number;
  /** Unix milliseconds, as a string. */
  lastSyncedAt: string | null;
  lastSyncedGameVersion: string | null;
}

/** One board being compared - the strip draws it, and so does its column. */
export interface Board {
  id: string;
  /** This board's world or data center. */
  scope: string | null;
}

/** Everything the UI needs, in one round trip. */
export interface Settings {
  /** Boards being compared, left to right - one column each. */
  boards: Board[];
  /** False once the board limit is reached - the "+" goes disabled. */
  canAddBoard: boolean;
  hotkey: string;
  recentItemIds: number[];
  /** Hearted item ids, so a row can draw a filled heart without asking. */
  favoriteItemIds: number[];
  catalog: CatalogInfo;
  catalogReady: boolean;
  /** Why the global hotkey isn't active, or `null` when it is. */
  hotkeyError: string | null;
}

export interface SyncSummary {
  totalItems: number;
  marketableItems: number;
}

export interface SyncProgress {
  stage: "items" | "marketable" | "wrote";
  count: number;
}

/** Mirror of `AppError::kind()` in `src-tauri/src/error.rs`. */
export type ErrorKind =
  | "catalog"
  | "universalis"
  | "xivapi"
  | "config"
  | "invalid"
  | "internal";

export interface AppError {
  kind: ErrorKind;
  message: string;
}

/**
 * Commands reject with the serialised `AppError`, but a bridge-level failure
 * (a command that doesn't exist, say) rejects with something else entirely.
 * Normalise both so callers only ever handle one shape.
 */
export function toAppError(error: unknown): AppError {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error
  ) {
    return error as AppError;
  }
  return { kind: "internal", message: String(error) };
}

// --- Commands ---------------------------------------------------------------

export const searchItems = (query: string, limit?: number) =>
  invoke<SearchResult[]>("search_items", { query, limit });

export const getPrice = (board: string, itemId: number, refresh = false) =>
  invoke<PriceData>("get_price", { board, itemId, refresh });

export const getSettings = () => invoke<Settings>("get_settings");

/** Seed a board's scope on first launch. A no-op once one is set. */
export const ensureMarketScope = (board: string, region: string) =>
  invoke<Settings>("ensure_market_scope", { board, region });

export const setMarketScope = (board: string, scope: string) =>
  invoke<Settings>("set_market_scope", { board, scope });

export const setHotkey = (hotkey: string) =>
  invoke<Settings>("set_hotkey", { hotkey });

export const listMarketScopes = () =>
  invoke<MarketScopes>("list_market_scopes");

export const recordRecentItem = (itemId: number) =>
  invoke<void>("record_recent_item", { itemId });

export const getRecentItems = () => invoke<Item[]>("get_recent_items");

export const clearRecentItems = () => invoke<void>("clear_recent_items");

/** Heart an item, or un-heart one already hearted. */
export const toggleFavorite = (itemId: number) =>
  invoke<Settings>("toggle_favorite", { itemId });

export const getFavoriteItems = () => invoke<Item[]>("get_favorite_items");

export const refreshCatalog = () => invoke<SyncSummary>("refresh_catalog");

/**
 * Add a column on the board one level wider than the rightmost one - a
 * world's data center, a data center's region.
 */
export const addBoard = () => invoke<Settings>("add_board");

/** Close a column. Rejects when it is the only board left. */
export const removeBoard = (board: string) =>
  invoke<Settings>("remove_board", { board });

export const hideOverlay = () => invoke<void>("hide_overlay");

export const getCatalogPath = () => invoke<string>("get_catalog_path");

/** Subscribe to catalog-sync progress. Matches `SYNC_PROGRESS_EVENT` in Rust. */
export const onSyncProgress = (
  handler: (progress: SyncProgress) => void,
): Promise<UnlistenFn> =>
  listen<SyncProgress>("catalog-sync-progress", (event) =>
    handler(event.payload),
  );

/**
 * XIVAPI serves item icons straight from the game's asset paths. The webview
 * loads them directly - no proxy, no caching layer of our own.
 */
export const iconUrl = (iconPath: string | null): string | null =>
  iconPath
    ? `https://v2.xivapi.com/api/asset?path=${encodeURIComponent(iconPath)}&format=png`
    : null;
