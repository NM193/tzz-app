import type { QueueItem } from "../lib/queue";

type Props = {
  items: QueueItem[];
  onReveal: (path: string) => void;
};

export function QueueList({ items, onReveal }: Props) {
  if (items.length === 0) return null;

  return (
    <ul className="queue">
      {items.map((item) => (
        <li key={item.id} className={`queue__row queue__row--${item.status}`}>
          <span className="queue__dot" aria-hidden="true" />
          <div className="queue__body">
            <span className="queue__label">{item.result?.title ?? item.label}</span>
            {item.error && <span className="queue__error">{item.error}</span>}
          </div>
          <div className="button-group">
            {item.result && (
              <button
                type="button"
                className="ghost-button"
                title={item.result.folder}
                onClick={() => onReveal(item.result!.folder)}
              >
                Folder
              </button>
            )}
            {item.result?.transcript?.filePath && (
              <button
                type="button"
                className="ghost-button"
                onClick={() => onReveal(item.result!.transcript!.filePath!)}
              >
                MD
              </button>
            )}
            {item.result?.transcript?.pdfPath && (
              <button
                type="button"
                className="ghost-button"
                onClick={() => onReveal(item.result!.transcript!.pdfPath!)}
              >
                PDF
              </button>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}
