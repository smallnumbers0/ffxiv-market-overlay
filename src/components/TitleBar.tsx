import { CloseIcon, SettingsIcon } from "./icons";
import { hideOverlay } from "../lib/tauriApi";

interface TitleBarProps {
  showSettings: boolean;
  onToggleSettings: () => void;
}

/**
 * The window has no decorations, so this bar is both the title and the drag
 * handle. `data-tauri-drag-region` is what makes the window follow the mouse.
 *
 * The board name lives in the tab strip below rather than here - with several
 * boards open, naming one of them up here would just be a second, sometimes
 * disagreeing, copy of the same fact.
 */
export function TitleBar({ showSettings, onToggleSettings }: TitleBarProps) {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-title" data-tauri-drag-region>
        Market
      </span>

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
