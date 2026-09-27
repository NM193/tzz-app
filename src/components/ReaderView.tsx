import { useEffect, useRef, useState } from "react";
import {
  errorMessage,
  openDocument,
  revealInFileManager,
  type LibraryEntry,
  type TranscriptDocument,
} from "../lib/api";
import { parts as split, type Scan } from "../lib/find";
import { drawRule } from "../lib/motion";

type Props = {
  entry: LibraryEntry;
  showScreens: boolean;
  onLoaded: (document: TranscriptDocument | null) => void;
  /** Set by the sidebar's contents when a chapter is chosen. */
  goTo: number | null;
  onArrived: () => void;
  /** Which chapter is at the top of the view. */
  onHere: (index: number) => void;
  find: string;
  /** Where every match is, numbered in reading order. */
  scan: Scan;
  /** The match to scroll to and light up. */
  hit: number;
};

/** Chapter headings are anchors; the sidebar scrolls the stage to them. */
export const chapterId = (index: number) => `chapter-${index}`;

/** One line, with the matches marked and numbered from `base`. */
function Line({ text, find, base }: { text: string; find: string; base: number }) {
  if (!find.trim()) return <>{text}</>;
  let n = base;
  return (
    <>
      {split(text, find).map((part, index) =>
        part.hit ? (
          <mark key={index} data-hit={n++}>
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </>
  );
}

export function ReaderView({
  entry,
  showScreens,
  onLoaded,
  goTo,
  onArrived,
  onHere,
  find,
  scan,
  hit,
}: Props) {
  const [doc, setDoc] = useState<TranscriptDocument | null>(null);
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setDoc(null);
    setError(null);
    onLoaded(null);

    if (!entry.markdownPath) {
      setError("This folder has no transcript to read.");
      return;
    }

    let stale = false;
    openDocument(entry.markdownPath)
      .then((loaded) => {
        if (stale) return;
        setDoc(loaded);
        onLoaded(loaded);
      })
      .catch((caught) => !stale && setError(errorMessage(caught)));

    return () => {
      stale = true;
    };
  }, [entry.path, entry.markdownPath]);

  // Each chapter's rule draws once, as everything else in the app does.
  useEffect(() => {
    if (!doc || !box.current) return;
    box.current
      .querySelectorAll<HTMLElement>("[data-id]")
      .forEach((el, index) => index < 12 && drawRule(el, index));
  }, [doc]);

  // Whichever chapter heading is nearest the top of the window is where you
  // are. Reported up so the contents can mark it.
  useEffect(() => {
    if (!doc || !box.current) return;
    const sections = [...box.current.querySelectorAll<HTMLElement>("[data-id]")];
    const observer = new IntersectionObserver(
      () => {
        let top: { index: number; y: number } | null = null;
        for (const section of sections) {
          const y = section.getBoundingClientRect().top;
          if (y > window.innerHeight) continue;
          if (!top || y > top.y) top = { index: Number(section.dataset.id), y };
        }
        if (top) onHere(top.index);
      },
      { threshold: 0, rootMargin: "0px 0px -80% 0px" },
    );
    sections.forEach((section) => observer.observe(section));
    return () => observer.disconnect();
  }, [doc]);

  // The current match is scrolled to rather than searched for again: every
  // match carries its number, so this is one lookup.
  useEffect(() => {
    if (!find.trim()) return;
    const found = document.querySelector<HTMLElement>(`[data-hit="${hit}"]`);
    if (!found) return;
    for (const other of document.querySelectorAll("[data-here-hit]")) {
      other.removeAttribute("data-here-hit");
    }
    found.setAttribute("data-here-hit", "");
    found.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "center",
    });
  }, [hit, find, doc, showScreens]);

  useEffect(() => {
    if (goTo === null) return;
    document.getElementById(chapterId(goTo))?.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth",
      block: "start",
    });
    onArrived();
  }, [goTo]);

  if (error) {
    return (
      <>
        <h1 className="title">{entry.name}</h1>
        <p className="notice notice--loud">{error}</p>
        <button type="button" className="ghost" onClick={() => revealInFileManager(entry.path)}>
          Show the folder in Finder
        </button>
      </>
    );
  }

  if (!doc) return <p className="empty">Opening…</p>;


  return (
    <article className="reader" ref={box}>
      <h1 className="title reader__title">{doc.title || entry.name}</h1>
      <p className="reader__meta">
        {[...doc.meta, `${doc.words.toLocaleString()} words`].join(" · ")}
      </p>

      {doc.chapters.map((chapter, index) => (
        <section key={index} id={chapterId(index)} data-id={index} className="reader__chapter">
          {chapter.title && (
            <h2 className="reader__heading">
              {chapter.title}
              {chapter.at && <span className="reader__at">{chapter.at}</span>}
            </h2>
          )}

          {chapter.blocks.map((block, k) => {
            const base = scan.offsets[index]?.[k] ?? 0;

            if (block.kind === "screen") {
              return showScreens ? (
                <p key={k} className="reader__screen">
                  <span className="reader__stamp">{block.at}</span>
                  <Line text={block.text} find={find} base={base} />
                </p>
              ) : null;
            }

            return (
              <p key={k} className="reader__said">
                {block.kind === "said" && <span className="reader__stamp">{block.at}</span>}
                <Line text={block.text} find={find} base={base} />
              </p>
            );
          })}
        </section>
      ))}
    </article>
  );
}
