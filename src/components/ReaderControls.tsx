import { revealInFileManager, type LibraryEntry, type TranscriptDocument } from "../lib/api";
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
}: Props) {
  return (
    <div className="side__tools">
      <button type="button" className="ghost ghost--small" onClick={onBack}>
        ← Library
      </button>

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
                    {chapter.at && <i>{chapter.at}</i>}
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
