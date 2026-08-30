import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  checkDependencies,
  combineTranscripts,
  defaultWhisperModel,
  errorMessage,
  onProgress,
  openOutputFolder,
  probeVideo,
  revealInFileManager,
  runJob,
  transcribeFile,
  type AudioQuality,
  type TranscriptFormat,
  type CombineResult,
  type DependencyStatus,
  type JobResult,
  type Stage,
} from "./lib/api";
import { LinkField } from "./components/LinkField";
import { RecorderBar } from "./components/RecorderBar";
import { QueueList } from "./components/QueueList";
import {
  fileItem,
  pendingLink,
  summarise,
  urlItem,
  type PendingLink,
  type QueueItem,
} from "./lib/queue";

const STAGE_LABEL: Record<Stage, string> = {
  idle: "Ready",
  probing: "Reading video",
  downloading: "Downloading audio",
  converting: "Encoding MP3",
  reading: "Reading the screen",
  transcribing: "Transcribing",
  done: "Finished",
};

const SEGMENT_COUNT = 32;

// Mirrors AUDIO_EXTENSIONS in src-tauri/src/commands/media.rs.
const AUDIO_EXTENSIONS = [
  "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm",
];

// Persisted in localStorage so the app remembers your setup between launches.
const STORAGE_KEY = "yt-mp3.settings.v1";

type Settings = {
  outDir: string;
  wantTranscript: boolean;
  transcriptLangs: string[];
  audioQuality: AudioQuality;
  whisperModelPath: string;
  transcriptFormat: TranscriptFormat;
  combineQueue: boolean;
  reviewBeforeStart: boolean;
  readScreen: boolean;
  keepVideo: boolean;
};

const DEFAULT_SETTINGS: Settings = {
  outDir: "",
  wantTranscript: true,
  transcriptLangs: ["sr", "en"],
  audioQuality: "v0",
  whisperModelPath: "",
  transcriptFormat: "md",
  combineQueue: true,
  reviewBeforeStart: true,
  readScreen: false,
  keepVideo: false,
};

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export default function App() {
  const [links, setLinks] = useState<PendingLink[]>([]);
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [combined, setCombined] = useState<CombineResult | null>(null);
  // Work waiting on a look at the options, so a setting is not remembered
  // only once the hour-long job is already running.
  const [pendingStart, setPendingStart] = useState<QueueItem[] | null>(null);
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const [showSettings, setShowSettings] = useState(false);
  const [deps, setDeps] = useState<DependencyStatus[]>([]);

  const [stage, setStage] = useState<Stage>("idle");
  const [percent, setPercent] = useState<number | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<JobResult | null>(null);
  const [copied, setCopied] = useState(false);
  const [dragging, setDragging] = useState(false);

  const running = stage !== "idle" && stage !== "done";
  const stopRequested = useRef(false);
  // The queue is a ref so files dropped mid-run join the run in progress.
  const pending = useRef<QueueItem[]>([]);
  const draining = useRef(false);

  useEffect(() => {
    checkDependencies().then(setDeps).catch(() => setDeps([]));
  }, []);

  // Fill in a model found on disk, but never override a deliberate choice.
  useEffect(() => {
    if (settings.whisperModelPath) return;
    defaultWhisperModel()
      .then((path) => {
        if (!path) return;
        setSettings((prev) =>
          prev.whisperModelPath ? prev : { ...prev, whisperModelPath: path },
        );
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = onProgress((event) => {
      setStage(event.stage);
      setPercent(event.percent);
      // Progress ticks carry no detail; they must not wipe the file name.
      setDetail((prev) => event.detail ?? prev);
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  // Re-subscribed when `running` or the model path changes, so the handler
  // never closes over stale state.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragging(true);
      } else if (event.payload.type === "leave") {
        setDragging(false);
      } else if (event.payload.type === "drop") {
        setDragging(false);
        if (event.payload.paths.length > 0) {
          requestStart(event.payload.paths.map((path) => fileItem(path)));
        }
      }
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, [settings]);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  useEffect(() => {
    if (!showSettings) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") cancelSettings();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showSettings]);

  const missing = useMemo(
    () => deps.filter((d) => !d.found && d.name !== "whisper-cli"),
    [deps],
  );
  const whisperReady = useMemo(
    () => deps.some((d) => d.name === "whisper-cli" && d.found),
    [deps],
  );

  async function runOne(item: QueueItem): Promise<JobResult> {
    if (item.kind === "file") {
      return transcribeFile({
        path: item.target,
        whisperModelPath: settings.whisperModelPath || null,
        language: null,
        transcriptFormat: settings.transcriptFormat,
        readScreen: settings.readScreen,
      });
    }
    return runJob({
      url: item.target,
      outDir: settings.outDir || null,
      wantTranscript: settings.wantTranscript,
      transcriptLangs: settings.transcriptLangs,
      audioQuality: settings.audioQuality,
      whisperModelPath: settings.whisperModelPath || null,
      transcriptFormat: settings.transcriptFormat,
      readScreen: settings.readScreen,
      keepVideo: settings.keepVideo,
    });
  }

  // One at a time on purpose: the GPU is already saturated by a single Whisper
  // run, so a second job would only slow the first one down.
  function requestStart(items: QueueItem[]) {
    if (items.length === 0) return;
    if (!settings.reviewBeforeStart || draining.current) {
      void enqueue(items);
      return;
    }
    setPendingStart(items);
    setShowSettings(true);
  }

  function confirmStart() {
    const items = pendingStart;
    setPendingStart(null);
    setShowSettings(false);
    if (items) void enqueue(items);
  }

  function cancelSettings() {
    setPendingStart(null);
    setShowSettings(false);
  }

  async function enqueue(items: QueueItem[]) {
    if (items.length === 0) return;

    if (draining.current) {
      // Dropping more files while a job runs used to be ignored silently.
      setQueue((prev) => [...prev, ...items]);
      pending.current = [...pending.current, ...items];
      return;
    }

    setError(null);
    setResult(null);
    setCopied(false);
    setCombined(null);
    setQueue(items);
    pending.current = [...items];

    await drainQueue();
  }

  async function drainQueue() {
    draining.current = true;
    stopRequested.current = false;

    // Collected locally: a state updater must stay pure, and StrictMode calls
    // it twice to prove it. Combining from inside one ran the whole thing twice.
    const finished: QueueItem[] = [];

    while (!stopRequested.current) {
      const item = pending.current.shift();
      if (!item) break;

      setQueue((prev) =>
        prev.map((q) => (q.id === item.id ? { ...q, status: "running" } : q)),
      );
      setStage(item.kind === "file" ? "transcribing" : "probing");
      setPercent(null);
      setDetail(item.label);

      try {
        const job = await runOne(item);
        finished.push({ ...item, status: "done", result: job });
        setQueue((prev) =>
          prev.map((q) => (q.id === item.id ? { ...q, status: "done", result: job } : q)),
        );
        setResult(job);
      } catch (caught) {
        // A dead link must not take the other nine down with it.
        setQueue((prev) =>
          prev.map((q) =>
            q.id === item.id ? { ...q, status: "failed", error: errorMessage(caught) } : q,
          ),
        );
      }
    }

    draining.current = false;
    pending.current = [];
    setStage("done");
    setPercent(100);
    setDetail(null);

    await combineFinished(finished);
  }

  // Back to how the app looks on a fresh launch. Settings are kept: they
  // survive a restart too.
  function resetAll() {
    if (running) return;
    pending.current = [];
    setLinks([]);
    setQueue([]);
    setResult(null);
    setCombined(null);
    setError(null);
    setCopied(false);
    setStage("idle");
    setPercent(null);
    setDetail(null);
  }

  // A queue of one is just a job; there is nothing to stitch.
  async function combineFinished(items: QueueItem[]) {
    const sections = items
      .map((item) => ({ item, markdown: item.result?.transcript?.markdown }))
      .filter((entry) => entry.markdown)
      .map(({ item, markdown }) => ({
        title: item.result?.title ?? item.label,
        source: item.target,
        markdown: markdown!,
      }));

    if (!settings.combineQueue || sections.length < 2) return;

    try {
      setCombined(
        await combineTranscripts({
          sections,
          outDir: settings.outDir || null,
          transcriptFormat: settings.transcriptFormat,
        }),
      );
    } catch (caught) {
      // The per-video files are already saved; this one is a bonus.
      setError(errorMessage(caught));
    }
  }

  /// Titles are read in the background; a link stays usable either way.
  function addLinks(urls: string[]) {
    setLinks((prev) => {
      const known = new Set(prev.map((l) => l.url));
      const fresh = urls.filter((url) => !known.has(url)).map(pendingLink);

      for (const link of fresh) {
        probeVideo(link.url)
          .then((meta) =>
            setLinks((all) =>
              all.map((l) => (l.id === link.id ? { ...l, title: meta.title } : l)),
            ),
          )
          .catch(() =>
            setLinks((all) =>
              all.map((l) => (l.id === link.id ? { ...l, failed: true } : l)),
            ),
          );
      }

      return [...prev, ...fresh];
    });
  }

  // A finished recording joins the queue like any dropped file.
  function addRecording(path: string) {
    requestStart([fileItem(path)]);
  }

  function removeLink(id: string) {
    setLinks((prev) => prev.filter((l) => l.id !== id));
  }

  function startFromLinks() {
    if (links.length === 0) {
      setError("Paste at least one YouTube link.");
      return;
    }
    requestStart(links.map((l) => urlItem(l.url, l.title ?? l.url)));
  }

  async function pickAudioFile() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length > 0) requestStart(paths.map((path) => fileItem(path)));
  }

  async function pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setSettings((prev) => ({ ...prev, outDir: picked }));
    }
  }

  async function pickModel() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Whisper model", extensions: ["bin"] }],
    });
    if (typeof picked === "string") {
      setSettings((prev) => ({ ...prev, whisperModelPath: picked }));
    }
  }

  async function copyTranscript() {
    if (!result?.transcript) return;
    await navigator.clipboard.writeText(result.transcript.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  }

  const litSegments =
    percent === null
      ? running
        ? Math.floor(SEGMENT_COUNT / 3)
        : 0
      : Math.round((percent / 100) * SEGMENT_COUNT);

  return (
    <main className="app">
      <header className="masthead">
        <h1>Tzz App</h1>
        <div className="button-group">
          <button
            type="button"
            className="ghost-button"
            onClick={resetAll}
            disabled={running}
            title={running ? "Finish or stop the queue first" : "Clear everything"}
          >
            Reset
          </button>
          <button
            type="button"
            className="ghost-button"
            onClick={() => setShowSettings((isOpen) => !isOpen)}
          >
            Options
          </button>
        </div>
      </header>

      {missing.length > 0 && (
        <p className="notice notice--blocking">
          Missing: {missing.map((d) => d.name).join(", ")}. Install with{" "}
          <code>brew install yt-dlp ffmpeg</code>
        </p>
      )}

      <RecorderBar onSaved={addRecording} onError={setError} />

      <section className="deck">
        <span className="field-label">YouTube links</span>
        <div className="input-row">
          <LinkField
            links={links}
            onAdd={addLinks}
            onRemove={removeLink}
            disabled={running}
          />
          {running ? (
            <button
              type="button"
              className="primary-button"
              onClick={() => (stopRequested.current = true)}
            >
              Stop after this
            </button>
          ) : (
            <button
              type="button"
              className="primary-button"
              onClick={startFromLinks}
              disabled={links.length === 0 || missing.length > 0}
            >
              Download
            </button>
          )}
        </div>

        <div className="meter" role="progressbar" aria-valuenow={percent ?? 0}>
          <div className="meter__segments">
            {Array.from({ length: SEGMENT_COUNT }, (_, index) => (
              <span
                key={index}
                className={`segment ${index < litSegments ? "segment--lit" : ""} ${
                  stage === "done" ? "segment--done" : ""
                }`}
              />
            ))}
          </div>
          <div className="meter__readout">
            <span className="readout-stage">
              {STAGE_LABEL[stage]}
              {queue.length > 1 &&
                ` ${queue.filter((q) => q.status !== "waiting").length} / ${queue.length}`}
            </span>
            <span className="readout-value">
              {percent === null ? "--" : String(Math.round(percent)).padStart(3, " ")}%
            </span>
          </div>
        </div>

        {detail && <p className="detail">{detail}</p>}

        <div className={`dropzone ${dragging ? "dropzone--active" : ""}`}>
          <span>Or drop an audio file here to transcribe it</span>
          <button
            type="button"
            className="ghost-button"
            onClick={pickAudioFile}
            disabled={running}
          >
            Choose file
          </button>
        </div>

        {summarise(queue) && <p className="detail">{summarise(queue)}</p>}

        {combined && (
          <div className="queue__row queue__row--done">
            <span className="queue__dot" aria-hidden="true" />
            <div className="queue__body">
              <span className="queue__label">{combined.title}</span>
            </div>
            <div className="button-group">
              {combined.markdownPath && (
                <button
                  type="button"
                  className="ghost-button"
                  onClick={() => revealInFileManager(combined.markdownPath!)}
                >
                  MD
                </button>
              )}
              {combined.pdfPath && (
                <button
                  type="button"
                  className="ghost-button"
                  onClick={() => revealInFileManager(combined.pdfPath!)}
                >
                  PDF
                </button>
              )}
            </div>
          </div>
        )}
        <QueueList items={queue} onReveal={revealInFileManager} />
      </section>

      {showSettings && (
        // Clicking the backdrop closes; clicking inside must not.
        <div className="scrim" onClick={cancelSettings}>
          <section
            className="panel panel--modal"
            role="dialog"
            aria-modal="true"
            aria-label="Options"
            onClick={(event) => event.stopPropagation()}
          >
            <header className="panel__head">
              <span className="field-label">
                {pendingStart
                  ? `Before starting ${pendingStart.length} ${pendingStart.length === 1 ? "job" : "jobs"}`
                  : "Options"}
              </span>
              <div className="button-group">
                {pendingStart && (
                  <button type="button" className="ghost-button" onClick={cancelSettings}>
                    Cancel
                  </button>
                )}
                <button
                  type="button"
                  className={pendingStart ? "primary-button" : "ghost-button"}
                  onClick={pendingStart ? confirmStart : cancelSettings}
                >
                  {pendingStart ? "Start" : "Done"}
                </button>
              </div>
            </header>
          <div className="panel__row">
            <div>
              <span className="field-label">Save to</span>
              <p className="path">
                {settings.outDir || "~/Documents/Tzz Library"}
              </p>
              <span className="hint">Each video gets its own folder in here.</span>
            </div>
            <div className="button-group">
              <button
                type="button"
                className="ghost-button"
                onClick={() =>
                  openOutputFolder(settings.outDir || null).catch((caught) =>
                    setError(errorMessage(caught)),
                  )
                }
              >
                Open
              </button>
              <button type="button" className="ghost-button" onClick={pickFolder}>
                Change
              </button>
            </div>
          </div>

          <label className="panel__row panel__row--tap">
            <span className="field-label">Also get the transcript</span>
            <input
              type="checkbox"
              checked={settings.wantTranscript}
              onChange={(event) =>
                setSettings((prev) => ({
                  ...prev,
                  wantTranscript: event.target.checked,
                }))
              }
            />
          </label>

          <div className="panel__row panel__row--stacked">
            <span className="field-label">MP3 quality</span>
            <div className="chips">
              {(
                [
                  ["v0", "VBR V0"],
                  ["320", "CBR 320"],
                ] as [AudioQuality, string][]
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  className={`chip ${settings.audioQuality === value ? "chip--first" : ""}`}
                  onClick={() =>
                    setSettings((prev) => ({ ...prev, audioQuality: value }))
                  }
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          <label className="panel__row panel__row--tap">
            <span className="field-label">
              Check the options before each job
              <em className="hint">
                Opens this panel when work is added, so a setting is not
                remembered an hour too late.
              </em>
            </span>
            <input
              type="checkbox"
              checked={settings.reviewBeforeStart}
              onChange={(event) =>
                setSettings((prev) => ({ ...prev, reviewBeforeStart: event.target.checked }))
              }
            />
          </label>

          <label className="panel__row panel__row--tap">
            <span className="field-label">
              Read what is on screen
              <em className="hint">
                Adds slide and panel text to the transcript. Downloads the video
                and takes a few minutes more.
              </em>
            </span>
            <input
              type="checkbox"
              checked={settings.readScreen}
              onChange={(event) =>
                setSettings((prev) => ({ ...prev, readScreen: event.target.checked }))
              }
            />
          </label>

          <label className="panel__row panel__row--tap">
            <span className="field-label">
              Keep the video
              <em className="hint">Otherwise only the MP3 is saved.</em>
            </span>
            <input
              type="checkbox"
              checked={settings.keepVideo}
              onChange={(event) =>
                setSettings((prev) => ({ ...prev, keepVideo: event.target.checked }))
              }
            />
          </label>

          <label className="panel__row panel__row--tap">
            <span className="field-label">Combine a queue into one file</span>
            <input
              type="checkbox"
              checked={settings.combineQueue}
              onChange={(event) =>
                setSettings((prev) => ({ ...prev, combineQueue: event.target.checked }))
              }
            />
          </label>

          <div className="panel__row panel__row--stacked">
            <span className="field-label">Transcript file</span>
            <div className="chips">
              {(
                [
                  ["md", "Markdown"],
                  ["pdf", "PDF"],
                  ["both", "Both"],
                ] as [TranscriptFormat, string][]
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  className={`chip ${settings.transcriptFormat === value ? "chip--first" : ""}`}
                  onClick={() =>
                    setSettings((prev) => ({ ...prev, transcriptFormat: value }))
                  }
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          <div className="panel__row panel__row--stacked">
            <span className="field-label">Transcript language order</span>
            <div className="chips">
              {["sr", "en"].map((lang) => {
                const position = settings.transcriptLangs.indexOf(lang);
                return (
                  <button
                    key={lang}
                    type="button"
                    className={`chip ${position === 0 ? "chip--first" : ""}`}
                    onClick={() =>
                      setSettings((prev) =>
                        prev.transcriptLangs[0] === lang
                          ? prev
                          : {
                              ...prev,
                              transcriptLangs: [...prev.transcriptLangs].reverse(),
                            },
                      )
                    }
                  >
                    {lang.toUpperCase()}
                    {position === 0 && <em>first</em>}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="panel__row">
            <div>
              <span className="field-label">
                Whisper model {whisperReady ? "" : "(whisper-cli not installed)"}
              </span>
              <p className="path">
                {settings.whisperModelPath || "Not set -- captions only"}
              </p>
            </div>
            <button type="button" className="ghost-button" onClick={pickModel}>
              Choose
            </button>
          </div>
          </section>
        </div>
      )}

      {error && <p className="notice notice--error">{error}</p>}

      {result && (
        <section className="result">
          <h2>{result.title}</h2>
          <button
            type="button"
            className="path path--link"
            onClick={() => revealInFileManager(result.audioPath)}
          >
            {result.audioPath}
          </button>

          {result.warnings.map((warning) => (
            <p key={warning} className="notice">
              {warning}
            </p>
          ))}

          {result.transcript && (
            <div className="transcript">
              <div className="transcript__head">
                <span className="field-label">
                  Transcript ({result.transcript.source}
                  {result.transcript.language ? ` / ${result.transcript.language}` : ""})
                </span>
                <div className="button-group">
                  {result.transcript.filePath && (
                    <button
                      type="button"
                      className="ghost-button"
                      onClick={() =>
                        revealInFileManager(result.transcript!.filePath!)
                      }
                    >
                      Show MD
                    </button>
                  )}
                  {result.transcript.pdfPath && (
                    <button
                      type="button"
                      className="ghost-button"
                      onClick={() =>
                        revealInFileManager(result.transcript!.pdfPath!)
                      }
                    >
                      Show PDF
                    </button>
                  )}
                  <button type="button" className="ghost-button" onClick={copyTranscript}>
                    {copied ? "Copied" : "Copy"}
                  </button>
                </div>
              </div>
              <p className="transcript__body">{result.transcript.text}</p>
            </div>
          )}
        </section>
      )}
    </main>
  );
}
