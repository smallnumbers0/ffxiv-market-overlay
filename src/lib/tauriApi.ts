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

export interface Settings {
  marketScope: string | null;
  hotkey: string;
  window: { x: number; y: number; width: number; height: number } | null;
  recentItemIds: number[];
  /** True while `marketScope` is an unconfirmed first-launch guess. */
  marketScopeIsGuess: boolean;
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

export const getPrice = (itemId: number, refresh = false) =>
  invoke<PriceData>("get_price", { itemId, refresh });

export const getSettings = () => invoke<Settings>("get_settings");

/** Seed a scope on first launch. A no-op once one is set. */
export const ensureMarketScope = (region: string) =>
  invoke<Settings>("ensure_market_scope", { region });

export const setMarketScope = (scope: string) =>
  invoke<Settings>("set_market_scope", { scope });

export const setHotkey = (hotkey: string) =>
  invoke<Settings>("set_hotkey", { hotkey });

export const listMarketScopes = () =>
  invoke<MarketScopes>("list_market_scopes");

export const recordRecentItem = (itemId: number) =>
  invoke<void>("record_recent_item", { itemId });

export const getRecentItems = () => invoke<Item[]>("get_recent_items");

export const clearRecentItems = () => invoke<void>("clear_recent_items");

export const refreshCatalog = () => invoke<SyncSummary>("refresh_catalog");

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
