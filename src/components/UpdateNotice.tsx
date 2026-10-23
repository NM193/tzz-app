import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorMessage } from "../lib/api";
import { SECTIONS, notesSince } from "../lib/releaseNotes";

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

  // Everything since the version running, so a skipped release is not a lost
  // one. A manifest without `changes`, a release made by hand say, still has
  // its plain notes.
  const versions = notesSince(update.rawJson, update.currentVersion);

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
        {versions.length > 0
          ? versions.map((entry) => (
              <div key={entry.version} className="update__version">
                {entry.version !== update.version && (
                  <span className="eyebrow">{entry.version}</span>
                )}
                {SECTIONS.map(
                  ([key, label]) =>
                    entry[key].length > 0 && (
                      <div key={key}>
                        <span className="eyebrow">{label}</span>
                        <ul>
                          {entry[key].map((line, index) => (
                            <li key={index}>{line}</li>
                          ))}
                        </ul>
                      </div>
                    ),
                )}
              </div>
            ))
          : update.body && <p>{update.body}</p>}
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
