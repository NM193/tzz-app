import { useCallback, useMemo, type RefObject } from "react";
import { revealInFileManager, type LibraryEntry } from "../lib/api";
import { convertFileSrc } from "@tauri-apps/api/core";
import { kindsOf } from "../lib/library";
import { cardEnter, cardExit, rowEnter, rowExit } from "../lib/motion";
import { libraryLabel, type Settings } from "../lib/settings";
import { useListTransitions } from "../lib/useList";
import type { Library } from "../lib/useLibrary";
import { CountRoll } from "./CountRoll";
import { DocIcon, MusicIcon, VideoIcon } from "./Icons";
import { Lede } from "./Lede";
import { Marked } from "./Marked";

type Props = {
  settings: Settings;
  library: Library;
  box: RefObject<HTMLOListElement>;
  /** Open a folder's transcript in the reader. */
  onOpen: (entry: LibraryEntry) => void;
};

function when(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/** Only the results. The controls live in the sidebar. */
export function LibraryView({ settings, library, box, onOpen }: Props) {
  const { entries, error, matching, query, layout, cols, rows } = library;

  const byPath = useMemo(
    () => new Map(matching.map((entry) => [entry.path, entry])),
    [matching],
  );

  const enter = useCallback(
    (el: HTMLElement, index: number) => (layout === "grid" ? cardEnter : rowEnter)(el, index),
    [layout],
  );
  const exit = useCallback(
    (el: HTMLElement) => (layout === "grid" ? cardExit : rowExit)(el),
    [layout],
  );

  const paths = useListTransitions(
    box,
    useMemo(() => matching.map((entry) => entry.path), [matching]),
    enter,
    exit,
    layout,
  );
  const shown = useMemo(
    () => paths.map((path) => byPath.get(path)).filter((e): e is LibraryEntry => !!e),
    [paths, byPath],
  );

  const resting = `Everything you have turned into notes, in ${libraryLabel(settings.outDir)}.`;
  const lede = query
    ? matching.length > 0
      ? `Everything you have turned into notes, matching “${query}”.`
      : `Nothing here matches “${query}” yet.`
    : resting;
  const ledeHeights = useMemo(() => [resting], [resting]);

  return (
    <>
      <h1 className="title">
        Library <em>(<CountRoll value={matching.length} />)</em>
      </h1>
      <Lede text={lede} measure={ledeHeights} />

      {error && <p className="notice notice--loud">{error}</p>}

      {entries !== null && matching.length === 0 && (
        <p className="empty">
          {library.all.length === 0
            ? "Nothing here yet. Convert a lecture and it will show up."
            : "Nothing matches that."}
        </p>
      )}

      {layout === "grid" ? (
        <ol className="grid" data-cols={cols} ref={box}>
          {shown.map((entry) => (
            <li key={entry.path} data-id={entry.path} className="card">
              <button
                type="button"
                className="card__cover"
                data-cover
                title={entry.markdownPath ? "Read the transcript" : entry.path}
                onClick={() =>
                  entry.markdownPath ? onOpen(entry) : revealInFileManager(entry.path)
                }
              >
                {entry.posterPath ? (
                  <img src={convertFileSrc(entry.posterPath)} alt="" loading="lazy" />
                ) : (
                  <span>
                    {entry.videoPath ? (
                      <VideoIcon size={22} />
                    ) : entry.audioPath ? (
                      <MusicIcon size={22} />
                    ) : (
                      <DocIcon size={22} />
                    )}
                  </span>
                )}
              </button>
              <span className="card__row" data-text>
                <span className="card__title">
                  <Marked text={entry.name} query={query} />
                </span>
                <span className="card__meta">{when(entry.modified)}</span>
              </span>
              <span className="tags" data-text>
                {kindsOf(entry).map((kind) => (
                  <span key={kind} className="tag">
                    {kind}
                  </span>
                ))}
              </span>
            </li>
          ))}
        </ol>
      ) : (
        <ol className="list" data-rows={rows} ref={box}>
          {shown.map((entry, index) => (
            <li key={entry.path} data-id={entry.path} className="row">
              <div className="row__in">
                <span className="row__n" data-part>
                  {String(index + 1).padStart(2, "0")}
                </span>
                <span className="row__title" data-part>
                  {entry.markdownPath ? (
                    <button type="button" className="row__open" onClick={() => onOpen(entry)}>
                      <Marked text={entry.name} query={query} />
                    </button>
                  ) : (
                    <Marked text={entry.name} query={query} />
                  )}
                </span>
                <span className="row__meta" data-part>
                  {when(entry.modified)}
                </span>
                <span className="tags" data-part>
                  {kindsOf(entry).map((kind) => (
                    <span key={kind} className="tag">
                      {kind}
                    </span>
                  ))}
                </span>
                <span className="row__files" data-part>
                  <button
                    type="button"
                    className="icon-button"
                    onClick={() => revealInFileManager(entry.path)}
                  >
                    Folder
                  </button>
                  {entry.markdownPath && (
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => revealInFileManager(entry.markdownPath!)}
                    >
                      MD
                    </button>
                  )}
                  {entry.pdfPath && (
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => revealInFileManager(entry.pdfPath!)}
                    >
                      PDF
                    </button>
                  )}
                </span>
              </div>
            </li>
          ))}
        </ol>
      )}
    </>
  );
}
