import { useState } from "react";
import { revealInFileManager } from "../lib/api";
import { fileItem } from "../lib/queue";
import type { Jobs } from "../lib/useJobs";
import { clock, type Recorder } from "../lib/useRecorder";
import { Player } from "./Player";

const SEGMENTS = 40;

type Props = {
  recorder: Recorder;
  jobs: Jobs;
};

export function RecordView({ recorder, jobs }: Props) {
  const lit = Math.round(recorder.level * SEGMENTS);
  // One at a time: the player is mounted for whichever recording is playing.
  const [playing, setPlaying] = useState<string | null>(null);

  return (
    <>
      <h1 className="title">Record</h1>
      <p className="lede">
        A call, a live lecture, your own voice. Pick two inputs and both are
        recorded into one file -- the machine's sound and you. The recording joins
        the queue as soon as you name it, and the menu bar icon starts it too.
      </p>

      <div className={`studio ${recorder.recording ? "studio--live" : ""}`}>
        <span className="clock">{clock(recorder.elapsed)}</span>
        <span className="bar__buttons">
          {recorder.inputs.length === 0 ? (
            <span className="row__state">No audio input found</span>
          ) : (
            <>
              <select
                className="ghost"
                aria-label="Input"
                value={recorder.selected ?? ""}
                disabled={recorder.recording}
                onMouseDown={recorder.refreshInputs}
                onChange={(event) => recorder.select(Number(event.target.value))}
              >
                {recorder.inputs.map((input) => (
                  <option key={input.index} value={input.index}>
                    {input.name}
                  </option>
                ))}
              </select>

              <span className="studio__plus" aria-hidden="true">
                +
              </span>

              <select
                className="ghost"
                aria-label="Second input"
                value={recorder.second ?? ""}
                disabled={recorder.recording}
                onMouseDown={recorder.refreshInputs}
                onChange={(event) =>
                  recorder.selectSecond(event.target.value === "" ? null : Number(event.target.value))
                }
              >
                <option value="">Nothing else</option>
                {recorder.inputs
                  .filter((input) => input.index !== recorder.selected)
                  .map((input) => (
                    <option key={input.index} value={input.index}>
                      {input.name}
                    </option>
                  ))}
              </select>
            </>
          )}
          <button
            type="button"
            className="cta"
            disabled={recorder.inputs.length === 0}
            onClick={recorder.toggle}
          >
            {recorder.recording ? "Stop" : "Record"}
          </button>
        </span>
      </div>

      <div
        className="level"
        role="meter"
        aria-label="Input level"
        aria-valuenow={Math.round(recorder.level * 100)}
      >
        {Array.from({ length: SEGMENTS }, (_, index) => (
          <i
            key={index}
            data-lit={index < lit ? "" : undefined}
            data-hot={index > SEGMENTS - 5 ? "" : undefined}
          />
        ))}
      </div>

      {recorder.routedTo && (
        <p className="notice">Sound is going through {recorder.routedTo} while recording.</p>
      )}

      {recorder.saved.length > 0 && (
        <div className="section">
          <h2>Saved this session</h2>
          <ol className="rows">
            {recorder.saved.map((path, index) => (
              <li key={path} className="row">
                <div className="row__in">
                  <span className="row__n">{String(index + 1).padStart(2, "0")}</span>
                  <span className="row__title">{path.split("/").pop()}</span>
                  <span className="row__state">Saved</span>
                  <span className="row__files">
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => setPlaying(playing === path ? null : path)}
                    >
                      {playing === path ? "Stop" : "Play"}
                    </button>
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => revealInFileManager(path)}
                    >
                      Folder
                    </button>
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => void jobs.enqueue([fileItem(path)])}
                    >
                      Transcribe
                    </button>
                  </span>
                </div>

                {playing === path && (
                  <Player key={path} path={path} onClose={() => setPlaying(null)} />
                )}
              </li>
            ))}
          </ol>
        </div>
      )}
    </>
  );
}
