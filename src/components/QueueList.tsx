import { useEffect, useRef } from "react";
import type { QueueItem } from "../lib/queue";
import { drawRule } from "../lib/motion";

type Props = {
  items: QueueItem[];
  onReveal: (path: string) => void;
};

const STATE: Record<QueueItem["status"], string> = {
  waiting: "Waiting",
  running: "Working",
  done: "Done",
  failed: "Failed",
};

/** The queue, as an index: number, name, what it is doing, where it went. */
export function QueueList({ items, onReveal }: Props) {
  const box = useRef<HTMLOListElement>(null);
  const drawn = useRef(new Set<string>());

  // Each new row draws its own rule once, staggered like the reference.
  useEffect(() => {
    const list = box.current;
    if (!list) return;
    let fresh = 0;
    for (const row of list.querySelectorAll<HTMLElement>("[data-id]")) {
      const id = row.dataset.id ?? "";
      if (drawn.current.has(id)) continue;
      drawn.current.add(id);
      drawRule(row, fresh++);
    }
  }, [items]);

  if (items.length === 0) return null;

  return (
    <ol className="rows" ref={box}>
      {items.map((item, index) => (
        <li key={item.id} data-id={item.id} className={`row row--${item.status}`}>
          <div className="row__in">
            <span className="row__n">{String(index + 1).padStart(2, "0")}</span>
            <span className="row__title">
              {item.result?.title ?? item.label}
              {item.error && <span className="row__error">{item.error}</span>}
            </span>
            <span className="row__state">{STATE[item.status]}</span>
            <span className="row__files">
              {item.result && (
                <button
                  type="button"
                  className="icon-button"
                  onClick={() => onReveal(item.result!.folder)}
                >
                  Folder
                </button>
              )}
              {item.result?.transcript?.filePath && (
                <button
                  type="button"
                  className="icon-button"
                  onClick={() => onReveal(item.result!.transcript!.filePath!)}
                >
                  MD
                </button>
              )}
              {item.result?.transcript?.pdfPath && (
                <button
                  type="button"
                  className="icon-button"
                  onClick={() => onReveal(item.result!.transcript!.pdfPath!)}
                >
                  PDF
                </button>
              )}
            </span>
          </div>
        </li>
      ))}
    </ol>
  );
}
