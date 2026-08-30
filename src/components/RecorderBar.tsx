import { useEffect, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  errorMessage,
  listAudioInputs,
  onRecording,
  saveRecording,
  setAudioInput,
  toggleRecording,
  type AudioInput,
} from "../lib/api";

type Props = {
  /** Called with the saved file's path once a recording has a name. */
  onSaved: (path: string) => void;
  onError: (message: string) => void;
};

const LEVEL_SEGMENTS = 14;

function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

/** Today's date and time, as a filename. */
function defaultName(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `Recording ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(
    now.getHours(),
  )}-${pad(now.getMinutes())}.mp3`;
}

/**
 * Always on screen, and deliberately independent of the queue: a recording can
 * start while a transcription is running, and neither waits for the other.
 *
 * State comes from events rather than from the button's own call, because the
 * menu bar icon can start and stop a recording with this window closed.
 */
export function RecorderBar({ onSaved, onError }: Props) {
  const [inputs, setInputs] = useState<AudioInput[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [recording, setRecording] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [level, setLevel] = useState(0);
  const [routedTo, setRoutedTo] = useState<string | null>(null);
  const tick = useRef<number | null>(null);

  function refreshInputs() {
    listAudioInputs()
      .then((found) => {
        setInputs(found);
        // Indexes shift when a device appears, so re-check the chosen one.
        setSelected((prev) =>
          found.some((input) => input.index === prev) ? prev : (found[0]?.index ?? null),
        );
      })
      .catch(() => setInputs([]));
  }

  // Read once at startup, then again whenever the menu is opened: plugging in
  // an interface or installing BlackHole must not need a restart.
  useEffect(refreshInputs, []);

  useEffect(() => {
    if (selected !== null) void setAudioInput(selected).catch(() => {});
  }, [selected]);

  useEffect(() => {
    const unlisten = onRecording({
      started: () => {
        setElapsed(0);
        setLevel(0);
        setRoutedTo(null);
        setRecording(true);
      },
      stopped: (result) => {
        setRecording(false);
        setLevel(0);
        setRoutedTo(null);
        void nameAndSave(result.tempPath);
      },
      level: setLevel,
      input: setSelected,
      output: setRoutedTo,
      error: onError,
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!recording) {
      if (tick.current) window.clearInterval(tick.current);
      return;
    }
    tick.current = window.setInterval(() => setElapsed((s) => s + 1), 1000);
    return () => {
      if (tick.current) window.clearInterval(tick.current);
    };
  }, [recording]);

  async function nameAndSave(tempPath: string) {
    try {
      const destination = await save({
        defaultPath: defaultName(),
        filters: [{ name: "MP3", extensions: ["mp3"] }],
      });

      if (!destination) {
        // Cancelling must never cost an hour of audio.
        onError(`Recording kept at ${tempPath} until you save it.`);
        return;
      }

      onSaved(await saveRecording(tempPath, destination));
    } catch (caught) {
      onError(errorMessage(caught));
    }
  }

  const lit = Math.round(level * LEVEL_SEGMENTS);

  return (
    <section className={`recorder ${recording ? "recorder--live" : ""}`}>
      <span className="recorder__dot" aria-hidden="true" />

      {recording ? (
        <span className="recorder__time">{clock(elapsed)}</span>
      ) : (
        <span className="field-label">Record</span>
      )}

      {inputs.length === 0 ? (
        <span className="recorder__input recorder__input--empty">No audio input found</span>
      ) : (
        <select
          className="recorder__input"
          value={selected ?? ""}
          disabled={recording}
          onMouseDown={refreshInputs}
          onChange={(event) => setSelected(Number(event.target.value))}
        >
          {inputs.map((input) => (
            <option key={input.index} value={input.index}>
              {input.name}
            </option>
          ))}
        </select>
      )}

      <div
        className="level"
        role="meter"
        aria-label="Input level"
        aria-valuenow={Math.round(level * 100)}
      >
        {Array.from({ length: LEVEL_SEGMENTS }, (_, index) => (
          <span
            key={index}
            className={`level__bar ${index < lit ? "level__bar--lit" : ""} ${
              index > LEVEL_SEGMENTS - 3 ? "level__bar--hot" : ""
            }`}
          />
        ))}
      </div>

      {routedTo && (
        <span className="recorder__routed" title={`Sound is going through ${routedTo}`}>
          via {routedTo}
        </span>
      )}

      <button type="button" className="ghost-button" onClick={() => void toggleRecording()}>
        {recording ? "Stop" : "Record"}
      </button>
    </section>
  );
}
