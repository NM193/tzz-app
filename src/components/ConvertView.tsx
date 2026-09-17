import { open } from "@tauri-apps/plugin-dialog";
import { errorMessage, openOutputFolder, revealInFileManager } from "../lib/api";
import { fileItem, urlItem, type PendingLink } from "../lib/queue";
import { libraryLabel, type Settings } from "../lib/settings";
import { STAGE_LABEL, type Jobs } from "../lib/useJobs";
import { Dropdown } from "./Dropdown";
import { ConvertArt, Hero } from "./Hero";
import { DocIcon, FolderIcon, InfoIcon, PlayIcon, StopIcon, UploadIcon } from "./Icons";
import { LinkField } from "./LinkField";
import { QueueList } from "./QueueList";

// Mirrors AUDIO_EXTENSIONS in src-tauri/src/commands/media.rs.
const FILE_EXTENSIONS = [
  "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm",
];

type Keep = "audio" | "video" | "transcript";
type TranscriptChoice = "md" | "pdf" | "screen";

type Props = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  jobs: Jobs;
  links: PendingLink[];
  onAddLinks: (urls: string[]) => void;
  onRemoveLink: (id: string) => void;
  onClearLinks: () => void;
  dragging: boolean;
  blocked: string | null;
};

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
  const keeps: Keep[] = [
    ...(settings.wantAudio ? (["audio"] as Keep[]) : []),
    ...(settings.keepVideo ? (["video"] as Keep[]) : []),
    ...(settings.wantTranscript ? (["transcript"] as Keep[]) : []),
  ];
  const nothingKept = keeps.length === 0;

  function toggleKeep(value: Keep) {
    if (value === "audio") update({ wantAudio: !settings.wantAudio });
    if (value === "video") update({ keepVideo: !settings.keepVideo });
    if (value === "transcript") update({ wantTranscript: !settings.wantTranscript });
  }

  const wantMd = settings.transcriptFormat !== "pdf";
  const wantPdf = settings.transcriptFormat !== "md";

  // Markdown and PDF are two ticks; the stored format is what they add up to.
  // The last tick cannot come off: a transcript with no file is not one.
  function pickTranscript(value: TranscriptChoice) {
    if (value === "screen") {
      update({ readScreen: !settings.readScreen });
      return;
    }
    const md = value === "md" ? !wantMd : wantMd;
    const pdf = value === "pdf" ? !wantPdf : wantPdf;
    if (!md && !pdf) return;
    update({ transcriptFormat: md && pdf ? "both" : md ? "md" : "pdf" });
  }

  async function pickFolder(value: "current" | "change" | "open") {
    if (value === "open") {
      openOutputFolder(settings.outDir || null).catch((caught) => jobs.setError(errorMessage(caught)));
      return;
    }
    if (value === "change") {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") update({ outDir: picked });
    }
  }

  async function chooseFiles() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Audio or video", extensions: FILE_EXTENSIONS }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length > 0) void jobs.enqueue(paths.map((path) => fileItem(path)));
  }

  function start() {
    if (links.length === 0) return;
    const items = links.map((l) => urlItem(l.url, l.title ?? l.url));
    onClearLinks();
    void jobs.enqueue(items);
  }

  const keepSummary =
    keeps.length === 0
      ? "Nothing"
      : keeps.map((k) => ({ audio: "MP3", video: "Video", transcript: "Transcript" })[k]).join(" + ");

  const formatLabel = [wantMd && "Markdown", wantPdf && "PDF"].filter(Boolean).join(" + ");
  const transcriptSummary = settings.wantTranscript
    ? `${formatLabel}${settings.readScreen ? ", screen" : ""}`
    : "Off";

  const done = jobs.queue.filter((q) => q.status === "done").length;
  const failed = jobs.queue.filter((q) => q.status === "failed").length;
  const total = jobs.queue.length;

  let footer: string;
  if (jobs.running) {
    footer = `${STAGE_LABEL[jobs.stage]}${total > 1 ? ` · ${done + failed + 1} of ${total}` : ""}`;
  } else if (jobs.stage === "done") {
    footer = failed === 0 ? `${done} finished` : `${done} finished, ${failed} failed`;
  } else if (links.length > 0) {
    footer = `${links.length} ${links.length === 1 ? "link" : "links"} ready`;
  } else {
    footer = "Paste a link or drop a file";
  }

  return (
    <>
      <Hero
        kicker="Convert"
        title={
          <>
            Lecture to <span className="accent">notes</span>
          </>
        }
        lead="Paste YouTube links or drop a recording. Get the MP3, a timestamped transcript and what was on screen."
        art={<ConvertArt />}
      />

      {blocked && (
        <p className="notice notice--blocking">
          {blocked}
        </p>
      )}

      <section className="glass intake">
        <LinkField links={links} onAdd={onAddLinks} onRemove={onRemoveLink} disabled={false} />

        <button
          type="button"
          className={`dropzone ${dragging ? "dropzone--active" : ""}`}
          onClick={chooseFiles}
        >
          <span className="dropzone__badge">
            <UploadIcon size={22} />
          </span>
          <strong>Drop audio or video here</strong>
          <span>or click to browse</span>
        </button>

        <QueueList items={jobs.queue} onReveal={revealInFileManager} />

        {jobs.combined && (
          <div className="card card--done card--combined">
            <span className="card__icon">
              <DocIcon />
            </span>
            <div className="card__body">
              <span className="card__title">{jobs.combined.title}</span>
              <span className="card__meta">One document for the whole queue</span>
            </div>
            <div className="card__actions">
              {jobs.combined.markdownPath && (
                <button
                  type="button"
                  className="icon-button"
                  onClick={() => revealInFileManager(jobs.combined!.markdownPath!)}
                >
                  <DocIcon size={16} />
                  <span>MD</span>
                </button>
              )}
              {jobs.combined.pdfPath && (
                <button
                  type="button"
                  className="icon-button"
                  onClick={() => revealInFileManager(jobs.combined!.pdfPath!)}
                >
                  <DocIcon size={16} />
                  <span>PDF</span>
                </button>
              )}
            </div>
          </div>
        )}
      </section>

      <section className="glass options">
        <Dropdown<Keep>
          label="Keep"
          summary={keepSummary}
          options={[
            { value: "audio", label: "Audio", hint: "MP3" },
            { value: "video", label: "Video file", hint: "about a gigabyte an hour" },
            { value: "transcript", label: "Transcript" },
          ]}
          selected={keeps}
          onPick={toggleKeep}
          multiple
        />
        <Dropdown<TranscriptChoice>
          label="Transcript"
          summary={transcriptSummary}
          disabled={!settings.wantTranscript}
          options={[
            { value: "md", label: "Markdown" },
            { value: "pdf", label: "PDF" },
            { value: "screen", label: "Read what is on screen", hint: "slides and panels; slower" },
          ]}
          selected={[
            ...(wantMd ? (["md"] as const) : []),
            ...(wantPdf ? (["pdf"] as const) : []),
            ...(settings.readScreen ? (["screen"] as const) : []),
          ]}
          onPick={pickTranscript}
          multiple
        />
        <Dropdown<"current" | "change" | "open">
          label="Save to"
          icon={<FolderIcon size={16} />}
          summary={libraryLabel(settings.outDir)}
          options={[
            { value: "current", label: libraryLabel(settings.outDir), hint: settings.outDir || "~/Documents" },
            { value: "change", label: "Choose another folder" },
            { value: "open", label: "Show in Finder" },
          ]}
          selected={["current"]}
          onPick={(value) => void pickFolder(value)}
        />
      </section>

      {jobs.error && <p className="notice notice--error">{jobs.error}</p>}
      {jobs.result?.warnings.map((warning) => (
        <p key={warning} className="notice">
          {warning}
        </p>
      ))}

      <footer className="actionbar">
        <div className="actionbar__status">
          {jobs.running ? (
            <div className="progress" role="progressbar" aria-valuenow={jobs.percent ?? 0}>
              <span
                className={`progress__fill ${jobs.percent === null ? "progress__fill--busy" : ""}`}
                style={{ width: `${jobs.percent ?? 30}%` }}
              />
            </div>
          ) : (
            <InfoIcon size={16} />
          )}
          <span className="actionbar__text">
            {footer}
            {jobs.running && jobs.percent !== null && ` · ${Math.round(jobs.percent)}%`}
            {jobs.running && jobs.detail && <small>{jobs.detail}</small>}
          </span>
        </div>

        <div className="actionbar__buttons">
          {(jobs.stage === "done" || (links.length > 0 && !jobs.running)) && (
            <button
              type="button"
              className="ghost-button"
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
              {jobs.queue.filter((q) => q.status === "waiting").length > 0 && (
                <button
                  type="button"
                  className="ghost-button"
                  title="Let this one finish, then stop"
                  onClick={jobs.stopAfterThis}
                >
                  Stop after this
                </button>
              )}
              <button
                type="button"
                className="cta cta--stop"
                title="Stop now and delete what this job has written"
                onClick={jobs.stop}
              >
                <StopIcon size={16} />
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
              <PlayIcon size={16} />
              {links.length > 1 ? `Convert ${links.length} videos` : "Convert"}
            </button>
          )}
        </div>
      </footer>
    </>
  );
}
