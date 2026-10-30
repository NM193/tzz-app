import { useEffect, useRef, useState } from "react";
import { libraryClashes, type Clash } from "../lib/api";
import { rowEnter, sheetEnter } from "../lib/motion";
import {
  asRun,
  refusalLine,
  refused as refusedRows,
  replaceLine,
  rowsFrom,
  stateLine,
  toCandidates,
  type Choice,
  type NameRow,
} from "../lib/naming";
import type { QueueItem } from "../lib/queue";
import { Segmented } from "./Controls";

type Props = {
  /** The batch about to join the queue, in order. */
  items: QueueItem[];
  /** The first check's answer for every item. */
  clashes: Clash[];
  outDir: string | null;
  onConfirm: (items: QueueItem[]) => void;
  onCancel: () => void;
};

/**
 * What to do about jobs whose folder the library already has: a new name,
 * replace what is there (all but the notes), or skip. Asked once, before the
 * queue starts, so a run never stops halfway to ask.
 *
 * The rows are fixed when the dialog opens. Every change checks the batch
 * again as it would run, and that only marks fields: a name the library or
 * another job already uses keeps Start waiting.
 */
export function NamePrompt({ items, clashes, outDir, onConfirm, onCancel }: Props) {
  const [rows, setRows] = useState<NameRow[]>(() => rowsFrom(clashes));
  const [latest, setLatest] = useState<Map<string, Clash>>(
    () => new Map(clashes.map((clash) => [clash.key, clash])),
  );
  const [refused, setRefused] = useState<Set<string>>(new Set());
  /** A check is under way; Start waits for its answer. */
  const [checking, setChecking] = useState(true);
  const edited = useRef<string | null>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const first = new Map(clashes.map((clash) => [clash.key, clash]));
  const ready = !checking && refused.size === 0;

  // Every change is checked again, a moment after typing stops.
  useEffect(() => {
    setChecking(true);
    let current = true;
    const timer = window.setTimeout(() => {
      libraryClashes(outDir, toCandidates(asRun(items, rows)))
        .then((found) => {
          if (!current) return;
          setLatest(new Map(found.map((clash) => [clash.key, clash])));
          setRefused(refusedRows(rows, found, edited.current));
        })
        // A check that cannot run must not hold the work back.
        .catch(() => current && setRefused(new Set()))
        .finally(() => current && setChecking(false));
    }, 150);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [rows, items, outDir]);

  useEffect(() => {
    if (!sheet.current) return;
    sheetEnter(sheet.current);
    sheet.current.focus();
  }, []);

  function confirm() {
    if (ready) onConfirm(asRun(items, rows));
  }

  // Escape cancels; Enter starts, from the field too. A focused button
  // answers Enter itself, so it is left alone.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
      if (event.key !== "Enter" || event.isComposing) return;
      if (event.target instanceof HTMLButtonElement) return;
      confirm();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function change(key: string, patch: Partial<NameRow>) {
    edited.current = key;
    setRows((prev) => prev.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  }

  return (
    <div className="prompt">
      <div
        ref={sheet}
        className="prompt__sheet"
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="names-title"
      >
        <h2 id="names-title" className="prompt__title">
          Already in the library
        </h2>
        <ul className="prompt__rows">
          {rows.map((row, index) => {
            const item = items.find((i) => i.id === row.key);
            const clash = first.get(row.key);
            if (!item || !clash) return null;
            return (
              <NameRowView
                key={row.key}
                row={row}
                index={index}
                item={item}
                first={clash}
                latest={latest.get(row.key)}
                refused={refused.has(row.key)}
                onChange={(patch) => change(row.key, patch)}
              />
            );
          })}
        </ul>
        <div className="prompt__actions">
          <button type="button" className="ghost ghost--small" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="cta cta--small" disabled={!ready} onClick={confirm}>
            Start
          </button>
        </div>
      </div>
    </div>
  );
}

function NameRowView({
  row,
  index,
  item,
  first,
  latest,
  refused,
  onChange,
}: {
  row: NameRow;
  index: number;
  item: QueueItem;
  /** What the folder held when the dialog opened; the row is about that. */
  first: Clash;
  /** The last check's answer, for why a typed name is refused. */
  latest: Clash | undefined;
  refused: boolean;
  onChange: (patch: Partial<NameRow>) => void;
}) {
  const ref = useRef<HTMLLIElement>(null);

  // Once, on arrival, like the language prompt's rows.
  useEffect(() => {
    if (ref.current) rowEnter(ref.current, index);
  }, []);

  const line = refused
    ? refusalLine(row, latest)
    : row.choice === "replace"
      ? replaceLine(first.holds)
      : row.choice === "skip"
        ? "Not run"
        : stateLine(item, first);

  return (
    // Only a New name row grows the third line, for its field.
    <li ref={ref} className={row.choice === "rename" ? "prompt__row prompt__row--naming" : "prompt__row"}>
      <span className="prompt__name" data-part>
        {item.label}
      </span>
      <span className="prompt__state" data-part data-refused={refused || undefined}>
        {line}
      </span>
      <span className="prompt__choice" data-part>
        <Segmented<Choice>
          label={`What to do with ${item.label}`}
          value={row.choice}
          onChange={(choice) => onChange({ choice })}
          options={[
            { value: "rename", label: "New name" },
            { value: "replace", label: "Replace", disabled: !first.replaceable },
            { value: "skip", label: "Skip" },
          ]}
        />
      </span>
      {row.choice === "rename" && (
        <label className="field prompt__field" data-refused={refused || undefined}>
          <input
            type="text"
            value={row.name}
            spellCheck={false}
            aria-label={`New name for ${item.label}`}
            onChange={(event) => onChange({ name: event.target.value })}
          />
        </label>
      )}
    </li>
  );
}
