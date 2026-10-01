import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorMessage } from "../lib/api";

/**
 * A new version, and what is in it.
 *
 * Asked for once, at launch, and never again while the app is open: an update
 * that interrupts an hour-long transcription to announce itself is worse than
 * one that waits until tomorrow. Nothing is downloaded until you say so.
 */
export function UpdateNotice() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [getting, setGetting] = useState(false);
  const [percent, setPercent] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    check()
      .then((found) => found && setUpdate(found))
      // No network, or no release yet: not worth saying anything about.
      .catch(() => {});
  }, []);

  if (!update || dismissed) return null;

  async function install() {
    if (!update) return;
    setGetting(true);
    setError(null);
    try {
      let had = 0;
      let total = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          had += event.data.chunkLength;
          if (total) setPercent((had / total) * 100);
        }
      });
      await relaunch();
    } catch (caught) {
      setError(errorMessage(caught));
      setGetting(false);
    }
  }

  return (
    <div className="update">
      <div className="update__text">
        <b>Version {update.version}</b>
        {update.body && <p>{update.body}</p>}
      </div>

      {error && <p className="notice notice--loud">{error}</p>}

      <div className="update__actions">
        {getting ? (
          <span className="bar__text">
            <span className="track">
              <i style={{ width: `${percent}%` }} />
            </span>
            {Math.round(percent)}%
          </span>
        ) : (
          <>
            <button type="button" className="ghost ghost--small" onClick={() => setDismissed(true)}>
              Later
            </button>
            <button type="button" className="cta cta--small" onClick={() => void install()}>
              Update
            </button>
          </>
        )}
      </div>
    </div>
  );
}
