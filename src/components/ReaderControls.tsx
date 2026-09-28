import { revealInFileManager, type LibraryEntry, type TranscriptDocument } from "../lib/api";
import type { Scan } from "../lib/find";
import { FindField } from "./FindField";
import { Check } from "./Controls";

type Props = {
  entry: LibraryEntry;
  doc: TranscriptDocument | null;
  showScreens: boolean;
  onShowScreens: (show: boolean) => void;
  onBack: () => void;
  onGoTo: (index: number) => void;
  /** Which chapter the reader is currently inside. */
  here: number;
  which: "transcript" | "notes";
  onWhich: (which: "transcript" | "notes") => void;
  find: string;
  onFind: (value: string) => void;
  found: Scan;
  hit: number;
  onStep: (by: number) => void;
};

/**
 * The document's own contents, in the sidebar, where the Library's controls
 * were. A transcript has forty chapters; a list beside the text is the only
 * way to move around one without scrolling for a minute.
 */
export function ReaderControls({
  entry,
  doc,
  showScreens,
  onShowScreens,
  onBack,
  onGoTo,
  here,
  which,
  onWhich,
  find,
  onFind,
  found,
  hit,
  onStep,
}: Props) {
  return (
    <div className="side__tools">
      <button type="button" className="ghost ghost--small" onClick={onBack}>
        ← Library
      </button>

      {entry.notesPath && entry.markdownPath && (
        <div className="segmented segmented--wide" role="radiogroup" aria-label="Document">
          <button
            type="button"
            role="radio"
            aria-checked={which === "transcript"}
            onClick={() => onWhich("transcript")}
          >
            Transcript
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={which === "notes"}
            onClick={() => onWhich("notes")}
          >
            Notes
          </button>
        </div>
      )}

      <FindField
        value={find}
        onChange={onFind}
        total={found.total}
        hit={hit}
        onStep={onStep}
      />

      {doc && doc.chapters.some((chapter) => chapter.title) && (
        <>
          <span className="eyebrow side__label">Contents</span>
          <ol className="toc">
            {doc.chapters.map((chapter, index) =>
              chapter.title ? (
                <li key={index}>
                  <button
                    type="button"
                    data-here={index === here ? "" : undefined}
                    onClick={() => onGoTo(index)}
                    title={chapter.title}
                  >
                    <span>{chapter.title}</span>
                    {found.perChapter[index] > 0 ? (
                      <i data-found>{found.perChapter[index]}</i>
                    ) : (
                      chapter.at && <i>{chapter.at}</i>
                    )}
                  </button>
                </li>
              ) : null,
            )}
          </ol>
        </>
      )}

      <div className="side__foot">
        <div className="checks">
          <Check
            on={showScreens}
            onToggle={() => onShowScreens(!showScreens)}
            label="What was on screen"
          />
        </div>
        <button
          type="button"
          className="ghost ghost--small"
          onClick={() => revealInFileManager(entry.markdownPath ?? entry.path)}
        >
          Show in Finder
        </button>
      </div>
    </div>
  );
}
