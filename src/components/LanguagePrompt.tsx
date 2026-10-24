import { useEffect, useRef, useState } from "react";
import { detectLanguage, type DetectedLanguage } from "../lib/api";
import { choiceFor, choicesFor, detectedLabel, languageName } from "../lib/language";
import { rowEnter, sheetEnter } from "../lib/motion";
import { Segmented } from "./Controls";

export type Confirmed = { path: string; language: string };

type Props = {
  /** Files waiting to be transcribed, in the order they arrived. */
  paths: string[];
  whisperModelPath: string | null;
  /** What a row selects when Whisper cannot say: the Caption language. */
  fallback: string;
  onConfirm: (files: Confirmed[]) => void;
  onCancel: () => void;
};

type Row = {
  path: string;
  /** Absent while Whisper listens; null when it could not tell. */
  detected?: DetectedLanguage | null;
  choice: string;
  /** Chosen by hand: a guess that arrives later must not undo it. */
  picked: boolean;
};

/**
 * Which language each file is spoken in, asked before it is transcribed.
 *
 * Whisper's guess is shown and selected, never trusted: on 2026-10-24 Serbian
 * over music came back as English. One file is heard at a time, so the dialog
 * never runs two Whispers at once (a guess still under way when you confirm
 * may overlap the transcription for a moment; accepted).
 *
 * Transcribe waits until every row has a guess or a choice of yours, so Enter
 * cannot send a file off on the fallback while Whisper is still listening.
 */
export function LanguagePrompt({ paths, whisperModelPath, fallback, onConfirm, onCancel }: Props) {
  const [rows, setRows] = useState<Row[]>([]);
  const sheet = useRef<HTMLDivElement>(null);
  const go = useRef<HTMLButtonElement>(null);
  // StrictMode runs effects twice; a file is still only heard once.
  const asked = useRef(new Set<string>());

  // A file that arrives while the dialog is open becomes another row.
  useEffect(() => {
    setRows((prev) => [
      ...prev,
      ...paths
        .filter((path) => !prev.some((row) => row.path === path))
        .map((path) => ({ path, choice: fallback, picked: false })),
    ]);
  }, [paths, fallback]);

  const listening = rows.find((row) => row.detected === undefined)?.path ?? null;
  const ready = rows.length > 0 && rows.every((row) => row.detected !== undefined || row.picked);

  useEffect(() => {
    if (!listening || asked.current.has(listening)) return;
    asked.current.add(listening);
    detectLanguage(listening, whisperModelPath)
      .then(
        (detected): DetectedLanguage | null => detected,
        () => null,
      )
      .then((detected) =>
        setRows((prev) =>
          prev.map((row) =>
            row.path !== listening
              ? row
              : {
                  ...row,
                  detected,
                  choice: row.picked || !detected ? row.choice : choiceFor(detected.code),
                },
          ),
        ),
      );
  }, [listening, whisperModelPath]);

  useEffect(() => {
    if (!sheet.current) return;
    sheetEnter(sheet.current);
    sheet.current.focus();
  }, []);

  // Enter confirms once there is something worth confirming.
  useEffect(() => {
    if (ready) go.current?.focus();
  }, [ready]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  function pick(path: string, choice: string) {
    setRows((prev) =>
      prev.map((row) => (row.path === path ? { ...row, choice, picked: true } : row)),
    );
  }

  return (
    <div className="prompt">
      <div
        ref={sheet}
        className="prompt__sheet"
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="prompt-title"
      >
        <h2 id="prompt-title" className="prompt__title">
          Which language is spoken?
        </h2>
        <ul className="prompt__rows">
          {rows.map((row, index) => (
            <PromptRow
              key={row.path}
              row={row}
              index={index}
              onPick={(choice) => pick(row.path, choice)}
            />
          ))}
        </ul>
        <div className="prompt__actions">
          <button type="button" className="ghost ghost--small" onClick={onCancel}>
            Cancel
          </button>
          <button
            ref={go}
            type="button"
            className="cta cta--small"
            disabled={!ready}
            onClick={() => onConfirm(rows.map(({ path, choice }) => ({ path, language: choice })))}
          >
            Transcribe
          </button>
        </div>
      </div>
    </div>
  );
}

function PromptRow({
  row,
  index,
  onPick,
}: {
  row: Row;
  index: number;
  onPick: (choice: string) => void;
}) {
  const ref = useRef<HTMLLIElement>(null);
  const name = row.path.split("/").pop() ?? row.path;

  // Once, on arrival: a row joining an open dialog rises in on its own.
  useEffect(() => {
    if (ref.current) rowEnter(ref.current, index);
  }, []);

  const state =
    row.detected === undefined
      ? "Listening…"
      : row.detected === null
        ? "Not detected"
        : detectedLabel(row.detected);

  return (
    <li ref={ref} className="prompt__row">
      <span className="prompt__name" data-part>
        {name}
      </span>
      <span className="prompt__state" data-part>
        {state}
      </span>
      <span className="prompt__choice" data-part>
        <Segmented<string>
          label={`Language spoken in ${name}`}
          value={row.choice}
          onChange={onPick}
          options={choicesFor(row.detected?.code ?? null).map((code) => ({
            value: code,
            label: languageName(code),
          }))}
        />
      </span>
    </li>
  );
}
