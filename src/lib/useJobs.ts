import { useEffect, useRef, useState } from "react";
import {
  cancelJob,
  combineTranscripts,
  errorMessage,
  onProgress,
  runJob,
  transcribeFile,
  type CombineResult,
  type JobResult,
  type Stage,
} from "./api";
import { summarise, tell } from "./notify";
import type { QueueItem } from "./queue";
import type { Settings } from "./settings";

export const STAGE_LABEL: Record<Stage, string> = {
  idle: "Ready",
  probing: "Reading video",
  downloading: "Downloading",
  converting: "Encoding MP3",
  reading: "Reading the screen",
  transcribing: "Transcribing",
  done: "Finished",
};

/**
 * The queue and its progress, kept out of the view so the sidebar and the
 * Convert screen read the same numbers.
 *
 * One job at a time on purpose: a single Whisper run already saturates the
 * GPU, so a second one would only slow the first.
 */
export function useJobs(settings: Settings) {
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [combined, setCombined] = useState<CombineResult | null>(null);
  const [stage, setStage] = useState<Stage>("idle");
  const [percent, setPercent] = useState<number | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<JobResult | null>(null);

  const running = stage !== "idle" && stage !== "done";
  const stopRequested = useRef(false);
  // Set by Stop: the current job is being torn down, not merely failing.
  const cancelled = useRef(false);
  // A ref so files dropped mid-run join the run in progress.
  const pending = useRef<QueueItem[]>([]);
  const draining = useRef(false);
  // The settings a job started with; edits mid-run apply to the next job.
  const latest = useRef(settings);
  latest.current = settings;

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

  async function runOne(item: QueueItem): Promise<JobResult> {
    const s = latest.current;
    if (item.kind === "file") {
      return transcribeFile({
        path: item.target,
        whisperModelPath: s.whisperModelPath || null,
        language: null,
        transcriptFormat: s.transcriptFormat,
        readScreen: s.readScreen,
      });
    }
    return runJob({
      url: item.target,
      outDir: s.outDir || null,
      wantAudio: s.wantAudio,
      wantTranscript: s.wantTranscript,
      transcriptLangs: s.transcriptLangs,
      audioQuality: s.audioQuality,
      whisperModelPath: s.whisperModelPath || null,
      transcriptFormat: s.transcriptFormat,
      readScreen: s.readScreen,
      keepVideo: s.keepVideo,
    });
  }

  async function enqueue(items: QueueItem[]) {
    if (items.length === 0) return;

    if (draining.current) {
      setQueue((prev) => [...prev, ...items]);
      pending.current = [...pending.current, ...items];
      return;
    }

    setError(null);
    setResult(null);
    setCombined(null);
    setQueue(items);
    pending.current = [...items];

    await drain();
  }

  async function drain() {
    draining.current = true;
    stopRequested.current = false;
    cancelled.current = false;

    // Collected locally: a state updater must stay pure, and StrictMode calls
    // it twice to prove it. Combining from inside one ran the whole thing twice.
    const finished: QueueItem[] = [];
    let failed = 0;

    while (!stopRequested.current) {
      const item = pending.current.shift();
      if (!item) break;

      setQueue((prev) => prev.map((q) => (q.id === item.id ? { ...q, status: "running" } : q)));
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
        if (cancelled.current) {
          // Stopped on purpose: the job and its files are gone, so is the row.
          setQueue((prev) => prev.filter((q) => q.id !== item.id));
          break;
        }
        // A dead link must not take the other nine down with it.
        failed += 1;
        setQueue((prev) =>
          prev.map((q) =>
            q.id === item.id ? { ...q, status: "failed", error: errorMessage(caught) } : q,
          ),
        );
      }
    }

    // Anything still waiting was skipped by "Stop after this".
    const skipped = new Set(pending.current.map((q) => q.id));
    if (skipped.size > 0) {
      setQueue((prev) => prev.filter((q) => !skipped.has(q.id)));
    }

    draining.current = false;
    pending.current = [];
    setDetail(null);

    // Said once for the whole run, not once per lecture: ten links would be
    // ten notifications, and by then you have stopped reading them.
    const said = cancelled.current ? null : summarise(finished.length, failed);
    if (latest.current.notify && said) void tell("Tzz App", said);

    if (cancelled.current) {
      // Nothing to combine and nothing to announce: the screen goes back to
      // how it was, with whatever finished before the stop still listed.
      setStage(finished.length > 0 ? "done" : "idle");
      setPercent(null);
      return;
    }

    setStage("done");
    setPercent(100);

    await combineFinished(finished);
  }

  // A queue of one is just a job; there is nothing to stitch.
  async function combineFinished(items: QueueItem[]) {
    const s = latest.current;
    const sections = items
      .map((item) => ({ item, markdown: item.result?.transcript?.markdown }))
      .filter((entry) => entry.markdown)
      .map(({ item, markdown }) => ({
        title: item.result?.title ?? item.label,
        source: item.target,
        markdown: markdown!,
      }));

    if (!s.combineQueue || sections.length < 2) return;

    try {
      setCombined(
        await combineTranscripts({
          sections,
          outDir: s.outDir || null,
          transcriptFormat: s.transcriptFormat,
        }),
      );
    } catch (caught) {
      // The per-video files are already saved; this one is a bonus.
      setError(errorMessage(caught));
    }
  }

  function stopAfterThis() {
    stopRequested.current = true;
  }

  /** Stop now. The current job's files are deleted; the rest of the queue is dropped. */
  function stop() {
    stopRequested.current = true;
    cancelled.current = true;
    setDetail("Stopping");
    void cancelJob().catch((caught) => setError(errorMessage(caught)));
  }

  // Back to how the app looks on a fresh launch. Settings are kept.
  function reset() {
    if (running) return;
    pending.current = [];
    setQueue([]);
    setResult(null);
    setCombined(null);
    setError(null);
    setStage("idle");
    setPercent(null);
    setDetail(null);
  }

  return {
    queue,
    combined,
    stage,
    percent,
    detail,
    error,
    result,
    running,
    enqueue,
    stopAfterThis,
    stop,
    reset,
    setError,
  };
}

export type Jobs = ReturnType<typeof useJobs>;
