import { useEffect, useMemo, useState } from "react";

import {
  clearRecentItems,
  getCatalogPath,
  listMarketScopes,
  onSyncProgress,
  refreshCatalog,
  setHotkey,
  setMarketScope,
  toAppError,
  type AppError,
  type MarketScopes,
  type Board,
  type Settings,
  type SyncProgress,
} from "../lib/tauriApi";
import { count, syncedAt } from "../lib/format";
import { guessRegion } from "../lib/region";
import { groupScopes } from "../lib/scopes";

interface SettingsPanelProps {
  settings: Settings;
  /** The column whose board this panel edits. */
  board: Board;
  onSettingsChange: (settings: Settings) => void;
  onCatalogRefreshed: () => void;
  onClose: () => void;
}

export function SettingsPanel({
  settings,
  board,
  onSettingsChange,
  onCatalogRefreshed,
  onClose,
}: SettingsPanelProps) {
  const [scopes, setScopes] = useState<MarketScopes | null>(null);
  const [scopesError, setScopesError] = useState<AppError | null>(null);
  const [hotkeyDraft, setHotkeyDraft] = useState(settings.hotkey);
  const [error, setError] = useState<AppError | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [catalogPath, setCatalogPath] = useState("");

  useEffect(() => setHotkeyDraft(settings.hotkey), [settings.hotkey]);

  useEffect(() => {
    let active = true;
    listMarketScopes()
      .then((result) => active && setScopes(result))
      .catch((cause) => active && setScopesError(toAppError(cause)));
    getCatalogPath().then((path) => active && setCatalogPath(path));
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const unlisten = onSyncProgress(setProgress);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  /** Worlds grouped under their data center, so the picker reads like the
   *  in-game world select rather than one flat list of 100+ names. The
   *  player's own region sorts first - see `groupScopes`. */
  const homeRegion = useMemo(() => guessRegion(), []);
  const grouped = useMemo(
    () => groupScopes(scopes, homeRegion),
    [scopes, homeRegion],
  );

  const recentCount = settings.recentItemIds.length;

  const run = async (action: () => Promise<Settings>) => {
    setError(null);
    try {
      onSettingsChange(await action());
    } catch (cause) {
      setError(toAppError(cause));
    }
  };

  const handleRefreshCatalog = async () => {
    setError(null);
    setSyncing(true);
    setProgress(null);
    try {
      await refreshCatalog();
      onCatalogRefreshed();
    } catch (cause) {
      setError(toAppError(cause));
    } finally {
      setSyncing(false);
    }
  };

  return (
    <section className="settings-panel">
      {/* "Done" rather than an x: the title bar already has an x, and there it
          hides the whole overlay. Two x's a few pixels apart meaning different
          things is how you get people closing the wrong one. */}
      <header className="settings-header">
        <h2 className="settings-title">Settings</h2>
        {board.scope && <span className="settings-board">{board.scope}</span>}
        <button
          type="button"
          className="button settings-done"
          onClick={onClose}
          title="Close settings (Esc)"
        >
          Done
        </button>
      </header>

      <div className="settings">
      <Field
        label="Home world or data center"
        hint="Sets this column only. The other columns keep their boards."
      >
        {scopesError ? (
          <p className="error-message">
            Couldn't load the world list: {scopesError.message}
          </p>
        ) : (
          <select
            className="input"
            value={board.scope ?? ""}
            disabled={!scopes}
            onChange={(event) =>
              void run(() => setMarketScope(board.id, event.target.value))
            }
          >
            <option value="" disabled>
              {scopes ? "Select a world..." : "Loading worlds..."}
            </option>
            {/* No whole-region entry: Universalis has to aggregate every
                world in a region to answer one, which is slow enough to 504.
                A data center is the widest board this app will ask for. */}
            {grouped.flatMap((group) =>
              group.dataCenters.map((dc) => (
                <optgroup key={dc.name} label={`${group.region} - ${dc.name}`}>
                  <option value={dc.name}>{dc.name} (whole data center)</option>
                  {dc.worldNames.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </optgroup>
              )),
            )}
          </select>
        )}
      </Field>

      <Field
        label="Overlay hotkey"
        hint="Example: CmdOrControl+Shift+M, Alt+M, F9."
      >
        {settings.hotkeyError && (
          <p className="error-message" role="alert">
            {settings.hotkeyError} Until you set one that works, open the
            overlay from the system tray icon.
          </p>
        )}
        <div className="field-row">
          <input
            className="input"
            value={hotkeyDraft}
            spellCheck={false}
            onChange={(event) => setHotkeyDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void run(() => setHotkey(hotkeyDraft));
            }}
          />
          <button
            type="button"
            className="button"
            disabled={hotkeyDraft.trim() === settings.hotkey}
            onClick={() => void run(() => setHotkey(hotkeyDraft))}
          >
            Apply
          </button>
        </div>
      </Field>

      <Field label="Item catalog">
        <p className="settings-detail">
          {count(settings.catalog.marketableItems)} marketable of{" "}
          {count(settings.catalog.totalItems)} items
          <br />
          Last synced: {syncedAt(settings.catalog.lastSyncedAt)}
          {settings.catalog.lastSyncedGameVersion && (
            <>
              <br />
              Game data version: {settings.catalog.lastSyncedGameVersion}
            </>
          )}
        </p>
        <button
          type="button"
          className="button"
          disabled={syncing}
          onClick={() => void handleRefreshCatalog()}
        >
          {syncing ? "Refreshing..." : "Refresh item database"}
        </button>
        {syncing && progress && (
          <p className="settings-detail">{describeProgress(progress)}</p>
        )}
        <p className="settings-hint">
          Run this after a game patch adds items. Takes about a minute.
        </p>
      </Field>

      <Field label="Recent searches">
        <button
          type="button"
          className="button"
          disabled={recentCount === 0}
          onClick={() => void clearRecentItems().then(onCatalogRefreshed)}
        >
          {recentCount === 0
            ? "No recent items"
            : `Clear ${recentCount} recent item${recentCount === 1 ? "" : "s"}`}
        </button>
      </Field>

      {error && (
        <p className="error-message" role="alert">
          {error.message}
        </p>
      )}

      {catalogPath && <p className="settings-path">Catalog: {catalogPath}</p>}
      </div>
    </section>
  );
}

function describeProgress(progress: SyncProgress): string {
  switch (progress.stage) {
    case "items":
      return `Downloading items: ${count(progress.count)}`;
    case "marketable":
      return `Checking marketability: ${count(progress.count)} items`;
    case "wrote":
      return `Saved ${count(progress.count)} items`;
  }
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="field">
      <label className="field-label">{label}</label>
      {children}
      {hint && <p className="settings-hint">{hint}</p>}
    </div>
  );
}
