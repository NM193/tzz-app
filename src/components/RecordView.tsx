import { revealInFileManager } from "../lib/api";
import { fileItem } from "../lib/queue";
import type { Jobs } from "../lib/useJobs";
import { clock, type Recorder } from "../lib/useRecorder";
import { Dropdown } from "./Dropdown";
import { Hero, RecordArt } from "./Hero";
import { DocIcon, FolderIcon, MicIcon, MusicIcon, StopIcon } from "./Icons";

const LEVEL_SEGMENTS = 28;

type Props = {
  recorder: Recorder;
  jobs: Jobs;
};

export function RecordView({ recorder, jobs }: Props) {
  const lit = Math.round(recorder.level * LEVEL_SEGMENTS);
  const current = recorder.inputs.find((i) => i.index === recorder.selected);

  return (
    <>
      <Hero
        kicker="Record"
        title={
          <>
            Capture <span className="accent">what you hear</span>
          </>
        }
        lead="A call, a live lecture, your own voice. The recording lands in the queue as soon as you name it."
        art={<RecordArt live={recorder.recording} />}
      />

      <section className={`glass studio ${recorder.recording ? "studio--live" : ""}`}>
        <div className="studio__top">
          <div className="studio__clock">
            <span className="studio__dot" aria-hidden="true" />
            <time>{clock(recorder.elapsed)}</time>
          </div>
          <div className="studio__input" onMouseDown={recorder.refreshInputs}>
            {recorder.inputs.length === 0 ? (
              <span className="row__hint">No audio input found</span>
            ) : (
              <Dropdown<string>
                label="Input"
                icon={<MicIcon size={16} />}
                summary={current?.name ?? "Choose an input"}
                options={recorder.inputs.map((input) => ({
                  value: String(input.index),
                  label: input.name,
                }))}
                selected={recorder.selected === null ? [] : [String(recorder.selected)]}
                onPick={(value) => recorder.select(Number(value))}
                disabled={recorder.recording}
              />
            )}
          </div>
        </div>

        <div className="level" role="meter" aria-label="Input level" aria-valuenow={Math.round(recorder.level * 100)}>
          {Array.from({ length: LEVEL_SEGMENTS }, (_, index) => (
            <span
              key={index}
              className={`level__bar ${index < lit ? "level__bar--lit" : ""} ${
                index > LEVEL_SEGMENTS - 4 ? "level__bar--hot" : ""
              }`}
            />
          ))}
        </div>

        <div className="studio__bottom">
          <span className="row__hint">
            {recorder.routedTo
              ? `Sound is going through ${recorder.routedTo}`
              : recorder.recording
                ? "Recording"
                : "The menu bar icon starts and stops this too."}
          </span>
          <button
            type="button"
            className={`cta ${recorder.recording ? "cta--stop" : "cta--live"}`}
            disabled={recorder.inputs.length === 0}
            onClick={recorder.toggle}
          >
            {recorder.recording ? <StopIcon size={16} /> : <MicIcon size={16} />}
            {recorder.recording ? "Stop" : "Record"}
          </button>
        </div>
      </section>

      {recorder.saved.length > 0 && (
        <section className="glass">
          <h2 className="section-title">Saved this session</h2>
          <ul className="cards">
            {recorder.saved.map((path) => (
              <li key={path} className="card card--done">
                <span className="card__icon">
                  <MusicIcon />
                </span>
                <div className="card__body">
                  <span className="card__title">{path.split("/").pop()}</span>
                  <span className="card__meta">{path}</span>
                </div>
                <div className="card__actions">
                  <button
                    type="button"
                    className="icon-button"
                    title="Show in Finder"
                    onClick={() => revealInFileManager(path)}
                  >
                    <FolderIcon size={16} />
                  </button>
                  <button
                    type="button"
                    className="icon-button"
                    title="Transcribe again"
                    onClick={() => void jobs.enqueue([fileItem(path)])}
                  >
                    <DocIcon size={16} />
                    <span>Transcribe</span>
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}
