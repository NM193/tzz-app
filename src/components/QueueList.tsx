import type { QueueItem } from "../lib/queue";
import { DocIcon, FolderIcon, LinkIcon, MusicIcon, VideoIcon } from "./Icons";

type Props = {
  items: QueueItem[];
  onReveal: (path: string) => void;
};

const STATUS_TEXT: Record<QueueItem["status"], string> = {
  waiting: "Waiting",
  running: "Working",
  done: "Done",
  failed: "Failed",
};

/** One card per job: what it is, how far it got, and where it went. */
export function QueueList({ items, onReveal }: Props) {
  if (items.length === 0) return null;

  return (
    <ul className="cards">
      {items.map((item) => (
        <li key={item.id} className={`card card--${item.status}`}>
          <span className="card__icon">
            {item.kind === "url" ? <LinkIcon /> : <MusicIcon />}
          </span>
          <div className="card__body">
            <span className="card__title">{item.result?.title ?? item.label}</span>
            <span className="card__meta">
              {item.error ?? STATUS_TEXT[item.status]}
            </span>
          </div>
          {item.result && (
            <div className="card__actions">
              <button
                type="button"
                className="icon-button"
                title="Show folder"
                onClick={() => onReveal(item.result!.folder)}
              >
                <FolderIcon size={16} />
              </button>
              {item.result.audioPath && (
                <button
                  type="button"
                  className="icon-button"
                  title="Show MP3"
                  onClick={() => onReveal(item.result!.audioPath!)}
                >
                  <MusicIcon size={16} />
                </button>
              )}
              {item.result.videoPath && (
                <button
                  type="button"
                  className="icon-button"
                  title="Show video"
                  onClick={() => onReveal(item.result!.videoPath!)}
                >
                  <VideoIcon size={16} />
                </button>
              )}
              {item.result.transcript?.filePath && (
                <button
                  type="button"
                  className="icon-button"
                  title="Show Markdown"
                  onClick={() => onReveal(item.result!.transcript!.filePath!)}
                >
                  <DocIcon size={16} />
                  <span>MD</span>
                </button>
              )}
              {item.result.transcript?.pdfPath && (
                <button
                  type="button"
                  className="icon-button"
                  title="Show PDF"
                  onClick={() => onReveal(item.result!.transcript!.pdfPath!)}
                >
                  <DocIcon size={16} />
                  <span>PDF</span>
                </button>
              )}
            </div>
          )}
        </li>
      ))}
    </ul>
  );
}
