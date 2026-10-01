import { useEffect, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  errorMessage,
  listAudioInputs,
  onRecording,
  saveRecording,
  setAudioInput,
  setSecondInput,
  toggleRecording,
  type AudioInput,
} from "./api";

/** Today's date and time, as a filename. */
function defaultName(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `Recording ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(
    now.getHours(),
  )}-${pad(now.getMinutes())}.mp3`;
}

export function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

type Handlers = {
  /** Called with the saved file's path once a recording has a name. */
  onSaved: (path: string) => void;
  onError: (message: string) => void;
};

/**
 * Recording state, held at the top of the app: the sidebar shows it, the
 * Record screen drives it, and neither waits for a transcription.
 *
 * State comes from events rather than from the button's own call, because the
 * menu bar icon can start and stop a recording with this window closed.
 */
export function useRecorder({ onSaved, onError }: Handlers) {
  const [inputs, setInputs] = useState<AudioInput[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [second, setSecond] = useState<number | null>(null);
  const [recording, setRecording] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [level, setLevel] = useState(0);
  const [routedTo, setRoutedTo] = useState<string | null>(null);
  const [saved, setSaved] = useState<string[]>([]);
  const tick = useRef<number | null>(null);
  const handlers = useRef({ onSaved, onError });
  handlers.current = { onSaved, onError };

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

  // Recording the same device twice would double it, not add anything.
  useEffect(() => {
    const paired = second === selected ? null : second;
    if (paired !== second) setSecond(paired);
    void setSecondInput(paired).catch(() => {});
  }, [second, selected]);

  // A device that has been unplugged cannot stay chosen.
  useEffect(() => {
    if (second !== null && !inputs.some((input) => input.index === second)) setSecond(null);
  }, [inputs, second]);

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
      error: (message) => handlers.current.onError(message),
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
        handlers.current.onError(`Recording kept at ${tempPath} until you save it.`);
        return;
      }

      const path = await saveRecording(tempPath, destination);
      setSaved((prev) => [path, ...prev]);
      handlers.current.onSaved(path);
    } catch (caught) {
      handlers.current.onError(errorMessage(caught));
    }
  }

  return {
    inputs,
    selected,
    select: setSelected,
    second,
    selectSecond: setSecond,
    refreshInputs,
    recording,
    elapsed,
    level,
    routedTo,
    saved,
    toggle: () => void toggleRecording(),
  };
}

export type Recorder = ReturnType<typeof useRecorder>;
