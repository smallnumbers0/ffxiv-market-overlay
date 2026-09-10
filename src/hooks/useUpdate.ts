import { useCallback, useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/**
 * How long after launch to look for an update.
 *
 * Not immediately: the first thing a user wants when the overlay opens is a
 * search box that works, and the check competes with the catalog load and the
 * first price fetch for the same connection.
 */
const CHECK_DELAY_MS = 4000;

export type UpdateStage =
  | { status: "idle" }
  | { status: "available"; version: string }
  | { status: "installing"; percent: number | null }
  | { status: "failed"; message: string };

/**
 * Checks GitHub for a newer release once per launch, and installs it on
 * request.
 *
 * A failed check is deliberately silent - the overlay is opened mid-fight to
 * read prices, and "couldn't reach the update server" is never what the user
 * came for. A failure *during* an install is shown, because by then they asked
 * for something and deserve to know it didn't happen.
 */
export function useUpdate() {
  const [stage, setStage] = useState<UpdateStage>({ status: "idle" });
  const [update, setUpdate] = useState<Update | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(() => {
      check()
        .then((found) => {
          if (cancelled || !found) return;
          setUpdate(found);
          setStage({ status: "available", version: found.version });
        })
        .catch((cause) => {
          // Offline, GitHub down, or a release with no manifest yet. None of
          // those are the user's problem right now.
          console.warn("update check failed:", cause);
        });
    }, CHECK_DELAY_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  const install = useCallback(async () => {
    if (!update) return;
    setStage({ status: "installing", percent: null });

    let total = 0;
    let downloaded = 0;
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            setStage({ status: "installing", percent: total > 0 ? 0 : null });
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            // A server that sends no content-length leaves nothing to divide
            // by; the banner falls back to an indeterminate message.
            setStage({
              status: "installing",
              percent:
                total > 0
                  ? Math.min(100, Math.round((downloaded / total) * 100))
                  : null,
            });
            break;
          case "Finished":
            setStage({ status: "installing", percent: 100 });
            break;
        }
      });
      await relaunch();
    } catch (cause) {
      setStage({
        status: "failed",
        message: cause instanceof Error ? cause.message : String(cause),
      });
    }
  }, [update]);

  const dismiss = useCallback(() => setStage({ status: "idle" }), []);

  return { stage, install, dismiss };
}
