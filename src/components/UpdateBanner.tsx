import { CloseIcon } from "./icons";
import type { UpdateStage } from "../hooks/useUpdate";

interface UpdateBannerProps {
  stage: UpdateStage;
  onInstall: () => void;
  onDismiss: () => void;
}

/**
 * A one-line offer to update, above the search box.
 *
 * Deliberately not a dialog: this window is opened over a running game to
 * answer one question quickly, and anything that has to be dismissed before
 * the search box works would be worse than never shipping updates at all.
 */
export function UpdateBanner({
  stage,
  onInstall,
  onDismiss,
}: UpdateBannerProps) {
  if (stage.status === "idle") return null;

  if (stage.status === "installing") {
    return (
      <p className="update-banner is-busy" role="status">
        {stage.percent === null
          ? "Downloading update..."
          : `Downloading update... ${stage.percent}%`}
        {stage.percent === 100 && " Restarting."}
      </p>
    );
  }

  if (stage.status === "failed") {
    return (
      <p className="update-banner is-failed" role="alert">
        Update failed: {stage.message}
        <button
          type="button"
          className="icon-button update-dismiss"
          onClick={onDismiss}
          title="Dismiss"
          aria-label="Dismiss"
        >
          <CloseIcon size={10} />
        </button>
      </p>
    );
  }

  return (
    <div className="update-banner" role="status">
      <span>
        Version <strong>{stage.version}</strong> is available.
      </span>
      <button type="button" className="update-action" onClick={onInstall}>
        Update and restart
      </button>
      <button
        type="button"
        className="icon-button update-dismiss"
        onClick={onDismiss}
        title="Not now"
        aria-label="Not now"
      >
        <CloseIcon size={10} />
      </button>
    </div>
  );
}
