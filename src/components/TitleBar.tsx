import { CloseIcon, SettingsIcon } from "./icons";
import { hideOverlay } from "../lib/tauriApi";

interface TitleBarProps {
  scope: string | null;
  showSettings: boolean;
  onToggleSettings: () => void;
}

/**
 * The window has no decorations, so this bar is both the title and the drag
 * handle. `data-tauri-drag-region` is what makes the window follow the mouse.
 */
export function TitleBar({
  scope,
  showSettings,
  onToggleSettings,
}: TitleBarProps) {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-title" data-tauri-drag-region>
        Market
      </span>

      {scope && (
        <span className="titlebar-scope" title="Prices are for this world or data center">
          {scope}
        </span>
      )}

      <div className="titlebar-actions">
        <button
          type="button"
          className={`icon-button${showSettings ? " is-active" : ""}`}
          onClick={onToggleSettings}
          title="Settings"
          aria-label="Settings"
          aria-pressed={showSettings}
        >
          <SettingsIcon />
        </button>
        <button
          type="button"
          className="icon-button"
          onClick={() => void hideOverlay()}
          title="Hide overlay (Esc)"
          aria-label="Hide overlay"
        >
          <CloseIcon />
        </button>
      </div>
    </header>
  );
}
