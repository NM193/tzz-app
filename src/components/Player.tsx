import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { clock } from "../lib/useRecorder";

type Props = {
  /** Absolute path to an audio file on disk. */
  path: string;
  onClose: () => void;
};

/**
 * Playing a file from the library without leaving for QuickTime.
 *
 * One of these exists at a time: the screen mounts it for whichever row is
 * playing and gives it a key, so choosing another row replaces the element and
 * the first one stops on its own. Nothing has to be told to stop.
 */
export function Player({ path, onClose }: Props) {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(true);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [gone, setGone] = useState(false);

  // Opened by a click, so this counts as a gesture and is allowed to start.
  useEffect(() => {
    audio.current?.play().catch(() => setPlaying(false));
  }, [path]);

  function toggle() {
    const el = audio.current;
    if (!el) return;
    if (el.paused) {
      el.play().catch(() => setPlaying(false));
      setPlaying(true);
    } else {
      el.pause();
      setPlaying(false);
    }
  }

  function seek(seconds: number) {
    const el = audio.current;
    if (!el) return;
    el.currentTime = seconds;
    setAt(seconds);
  }

  if (gone) {
    return (
      <p className="notice">
        That recording is no longer where it was.{" "}
        <button type="button" className="icon-button" onClick={onClose}>
          Close
        </button>
      </p>
    );
  }

  return (
    <div className="player">
      <audio
        ref={audio}
        src={convertFileSrc(path)}
        preload="metadata"
        onLoadedMetadata={(event) => setLength(event.currentTarget.duration || 0)}
        onTimeUpdate={(event) => setAt(event.currentTarget.currentTime)}
        onEnded={() => setPlaying(false)}
        onError={() => setGone(true)}
      />

      <button
        type="button"
        className="player__play"
        aria-label={playing ? "Pause" : "Play"}
        onClick={toggle}
      >
        {playing ? (
          <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
            <rect x="1.5" y="1" width="3" height="10" rx="1" />
            <rect x="7.5" y="1" width="3" height="10" rx="1" />
          </svg>
        ) : (
          <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
            <path d="M2.5 1.5v9l8-4.5z" />
          </svg>
        )}
      </button>

      <input
        className="player__scrub"
        type="range"
        min={0}
        max={Math.max(length, 0.1)}
        step={0.1}
        value={at}
        aria-label="Position"
        style={{ "--played": `${length ? (at / length) * 100 : 0}%` } as React.CSSProperties}
        onChange={(event) => seek(Number(event.target.value))}
      />

      <span className="player__time">
        {clock(Math.floor(at))}
        <em>{length ? clock(Math.floor(length)) : "--:--"}</em>
      </span>

      <button type="button" className="icon-button" onClick={onClose}>
        Close
      </button>
    </div>
  );
}
