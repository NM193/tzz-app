import { useEffect, useState } from "react";
import { errorMessage, listLibrary, openOutputFolder, revealInFileManager, type LibraryEntry } from "../lib/api";
import { libraryLabel, type Settings } from "../lib/settings";
import { Hero } from "./Hero";
import { DocIcon, FolderIcon, MusicIcon, VideoIcon } from "./Icons";

type Props = {
  settings: Settings;
  /** Bumped when a job finishes, so the list is never stale. */
  version: number;
};

function when(seconds: number): string {
  const date = new Date(seconds * 1000);
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export function LibraryView({ settings, version }: Props) {
  const [entries, setEntries] = useState<LibraryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listLibrary(settings.outDir || null)
      .then(setEntries)
      .catch((caught) => setError(errorMessage(caught)));
  }, [settings.outDir, version]);

  return (
    <>
      <Hero
        kicker="Library"
        title={
          <>
            Everything <span className="accent">so far</span>
          </>
        }
        lead={`One folder per video, in ${libraryLabel(settings.outDir)}.`}
      />

      <section className="glass">
        <div className="section-head">
          <h2 className="section-title">
            {entries === null ? "Reading" : `${entries.length} ${entries.length === 1 ? "folder" : "folders"}`}
          </h2>
          <button
            type="button"
            className="ghost-button"
            onClick={() => openOutputFolder(settings.outDir || null).catch((c) => setError(errorMessage(c)))}
          >
            <FolderIcon size={16} />
            Show in Finder
          </button>
        </div>

        {error && <p className="notice notice--error">{error}</p>}

        {entries && entries.length === 0 && (
          <p className="empty">Nothing here yet. Convert a video and it will show up.</p>
        )}

        {entries && entries.length > 0 && (
          <ul className="cards">
            {entries.map((entry) => (
              <li key={entry.path} className="card">
                <span className="card__icon">
                  {entry.videoPath ? <VideoIcon /> : entry.audioPath ? <MusicIcon /> : <DocIcon />}
                </span>
                <div className="card__body">
                  <span className="card__title">{entry.name}</span>
                  <span className="card__meta">{when(entry.modified)}</span>
                </div>
                <div className="card__actions">
                  <button type="button" className="icon-button" title="Show folder" onClick={() => revealInFileManager(entry.path)}>
                    <FolderIcon size={16} />
                  </button>
                  {entry.audioPath && (
                    <button type="button" className="icon-button" title="Show MP3" onClick={() => revealInFileManager(entry.audioPath!)}>
                      <MusicIcon size={16} />
                    </button>
                  )}
                  {entry.videoPath && (
                    <button type="button" className="icon-button" title="Show video" onClick={() => revealInFileManager(entry.videoPath!)}>
                      <VideoIcon size={16} />
                    </button>
                  )}
                  {entry.markdownPath && (
                    <button type="button" className="icon-button" onClick={() => revealInFileManager(entry.markdownPath!)}>
                      <DocIcon size={16} />
                      <span>MD</span>
                    </button>
                  )}
                  {entry.pdfPath && (
                    <button type="button" className="icon-button" onClick={() => revealInFileManager(entry.pdfPath!)}>
                      <DocIcon size={16} />
                      <span>PDF</span>
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
