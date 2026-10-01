import { open } from "@tauri-apps/plugin-dialog";
import { errorMessage, openOutputFolder, revealInFileManager } from "../lib/api";
import { fileItem, urlItem, type Pasted, type PendingLink } from "../lib/queue";
import { libraryLabel, type Settings } from "../lib/settings";
import { STAGE_LABEL, type Jobs } from "../lib/useJobs";
import { Check } from "./Controls";
import { LinkField } from "./LinkField";
import { QueueList } from "./QueueList";

// Mirrors AUDIO_EXTENSIONS in src-tauri/src/commands/media.rs.
const FILE_EXTENSIONS = [
  "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm",
];

type Props = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  jobs: Jobs;
  links: PendingLink[];
  onAddLinks: (pasted: Pasted[]) => void;
  onRemoveLink: (id: string) => void;
  onClearLinks: () => void;
  dragging: boolean;
  blocked: string | null;
};

/**
 * The screen the app is for. Every option is on it, as ticks -- there is no
 * menu to open and nothing to remember, because what you keep is the job.
 */
export function ConvertView({
  settings,
  update,
  jobs,
  links,
  onAddLinks,
  onRemoveLink,
  onClearLinks,
  dragging,
  blocked,
}: Props) {
  const wantMd = settings.transcriptFormat !== "pdf";
  const wantPdf = settings.transcriptFormat !== "md";
  const nothingKept = !settings.wantAudio && !settings.keepVideo && !settings.wantTranscript;

  // Markdown and PDF are two ticks; the stored format is what they add up to.
  // The last one cannot come off: a transcript with no file is not one.
  function setFormat(which: "md" | "pdf") {
    const md = which === "md" ? !wantMd : wantMd;
    const pdf = which === "pdf" ? !wantPdf : wantPdf;
    if (!md && !pdf) return;
    update({ transcriptFormat: md && pdf ? "both" : md ? "md" : "pdf" });
  }

  async function chooseFiles() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Audio or video", extensions: FILE_EXTENSIONS }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length > 0) void jobs.enqueue(paths.map((path) => fileItem(path)));
  }

  async function chooseFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") update({ outDir: picked });
  }

  function start() {
    if (links.length === 0) return;
    const items = links.map((l) => urlItem(l.url, l.title ?? l.url, l.title !== null));
    onClearLinks();
    void jobs.enqueue(items);
  }

  const done = jobs.queue.filter((q) => q.status === "done").length;
  const failed = jobs.queue.filter((q) => q.status === "failed").length;
  const total = jobs.queue.length;

  let footer: string;
  if (jobs.running) {
    footer = `${STAGE_LABEL[jobs.stage]}${jobs.percent === null ? "" : ` · ${Math.round(jobs.percent)}%`}${
      total > 1 ? ` · ${done + failed + 1} of ${total}` : ""
    }`;
  } else if (jobs.stage === "done") {
    footer = failed === 0 ? `${done} finished` : `${done} finished, ${failed} failed`;
  } else if (links.length > 0) {
    footer = `${links.length} ${links.length === 1 ? "link" : "links"} ready`;
  } else {
    footer = "Paste a link or drop a file";
  }

  return (
    <>
      <h1 className="title">Convert</h1>
      <p className="lede">
        Paste a lecture link or drop a recording. You get the audio, a timestamped
        transcript, and the text that was on screen.
      </p>

      {blocked && <p className="notice notice--loud">{blocked}</p>}

      <LinkField
        links={links}
        onAdd={onAddLinks}
        onRemove={onRemoveLink}
        disabled={false}
      />

      <button
        type="button"
        className={`drop ${dragging ? "drop--active" : ""}`}
        onClick={chooseFiles}
      >
        <b>Drop audio or video</b>
        <span>or click to choose a file</span>
      </button>

      <QueueList items={jobs.queue} onReveal={revealInFileManager} />

      {jobs.combined && (
        <p className="notice">
          One document for the whole queue: {jobs.combined.title}{" "}
          {jobs.combined.markdownPath && (
            <button
              type="button"
              className="icon-button"
              onClick={() => revealInFileManager(jobs.combined!.markdownPath!)}
            >
              MD
            </button>
          )}
          {jobs.combined.pdfPath && (
            <button
              type="button"
              className="icon-button"
              onClick={() => revealInFileManager(jobs.combined!.pdfPath!)}
            >
              PDF
            </button>
          )}
        </p>
      )}

      <div className="opts">
        <div>
          <span className="eyebrow">Keep</span>
          <div className="checks">
            <Check
              on={settings.wantAudio}
              onToggle={() => update({ wantAudio: !settings.wantAudio })}
              label="Audio"
              hint="MP3"
            />
            <Check
              on={settings.keepVideo}
              onToggle={() => update({ keepVideo: !settings.keepVideo })}
              label="Video file"
              hint="~1 GB an hour"
            />
            <Check
              on={settings.wantTranscript}
              onToggle={() => update({ wantTranscript: !settings.wantTranscript })}
              label="Transcript"
            />
          </div>
        </div>

        <div>
          <span className="eyebrow">Transcript</span>
          <div className="checks">
            <Check
              on={wantMd}
              disabled={!settings.wantTranscript}
              onToggle={() => setFormat("md")}
              label="Markdown"
            />
            <Check
              on={wantPdf}
              disabled={!settings.wantTranscript}
              onToggle={() => setFormat("pdf")}
              label="PDF"
            />
            <Check
              on={settings.readScreen}
              disabled={!settings.wantTranscript}
              onToggle={() => update({ readScreen: !settings.readScreen })}
              label="Read the screen"
              hint="slower"
            />
          </div>
        </div>

        <div>
          <span className="eyebrow">Save to</span>
          <div className="checks">
            <Check on onToggle={() => void chooseFolder()} label={libraryLabel(settings.outDir)} />
            <p className="path-line">{settings.outDir || "~/Documents/Tzz Library"}</p>
            <button
              type="button"
              className="icon-button"
              style={{ justifySelf: "start", marginTop: ".5rem" }}
              onClick={() =>
                openOutputFolder(settings.outDir || null).catch((caught) =>
                  jobs.setError(errorMessage(caught)),
                )
              }
            >
              Show in Finder
            </button>
          </div>
        </div>
      </div>

      {jobs.error && <p className="notice notice--loud">{jobs.error}</p>}
      {jobs.result?.warnings.map((warning) => (
        <p key={warning} className="notice">
          {warning}
        </p>
      ))}

      <div className="bar">
        <span className="bar__text">
          {jobs.running && (
            <span className="track">
              <i
                data-busy={jobs.percent === null ? "" : undefined}
                style={{ width: `${jobs.percent ?? 30}%` }}
              />
            </span>
          )}
          <span>
            {footer}
            {jobs.running && jobs.detail && <small>{jobs.detail}</small>}
          </span>
        </span>

        <span className="bar__buttons">
          {!jobs.running && (jobs.stage === "done" || links.length > 0) && (
            <button
              type="button"
              className="ghost"
              onClick={() => {
                jobs.reset();
                onClearLinks();
              }}
            >
              Clear
            </button>
          )}
          {jobs.running ? (
            <>
              {jobs.queue.some((q) => q.status === "waiting") && (
                <button type="button" className="ghost" onClick={jobs.stopAfterThis}>
                  Stop after this
                </button>
              )}
              <button
                type="button"
                className="cta"
                title="Stop now and delete what this job has written"
                onClick={jobs.stop}
              >
                Stop
              </button>
            </>
          ) : (
            <button
              type="button"
              className="cta"
              disabled={links.length === 0 || nothingKept || !!blocked}
              title={nothingKept ? "Choose something to keep first" : undefined}
              onClick={start}
            >
              {links.length > 1 ? `Convert ${links.length} lectures` : "Convert"}
            </button>
          )}
        </span>
      </div>
    </>
  );
}
