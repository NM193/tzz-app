import { useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorMessage } from "../lib/api";
import { stillPreferred } from "../lib/motion";
import { SECTIONS, notesSince } from "../lib/releaseNotes";

/** What the last look for a newer version found, for Settings to say. */
export type UpdateStatus =
  | { state: "checking" }
  | { state: "current" }
  | { state: "available"; version: string }
  | { state: "failed" };

type Props = {
  /** Bumped by Check for updates in Settings: look again, and show what is found. */
  asked: number;
  /** Must be stable -- a state setter -- or every render would look again. */
  onResult: (status: UpdateStatus) => void;
};

/**
 * A new version, and what is in it.
 *
 * Asked for at launch, and again only when you press Check for updates in
 * Settings: an update that interrupts an hour-long transcription to announce
 * itself is worse than one that waits until tomorrow. Nothing is downloaded
 * until you say so.
 */
export function UpdateNotice({ asked, onResult }: Props) {
  const [update, setUpdate] = useState<Update | null>(null);
  const [getting, setGetting] = useState(false);
  const [percent, setPercent] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    check()
      .then((found) => {
        setUpdate(found);
        // Asked for again: show it even if it was put off with Later.
        setDismissed(false);
        onResult(found ? { state: "available", version: found.version } : { state: "current" });
      })
      // No network, or no release yet: not worth interrupting anyone for.
      // Settings says so, for whoever asked.
      .catch(() => onResult({ state: "failed" }));
  }, [asked]);

  // Found because someone asked, from the bottom of Settings: bring it into view.
  useEffect(() => {
    if (asked > 0 && update && !dismissed) {
      box.current?.scrollIntoView({ block: "start", behavior: stillPreferred() ? "auto" : "smooth" });
    }
  }, [asked, update, dismissed]);

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
    <div ref={box} className="update">
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
