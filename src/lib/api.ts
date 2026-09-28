import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// These types mirror the Rust structs in src-tauri/src. Keep them in sync:
// the Rust side serialises everything as camelCase.

export type DependencyStatus = {
  name: string;
  found: boolean;
  path: string | null;
};

export type VideoMeta = {
  id: string;
  title: string;
  uploader: string | null;
  durationSeconds: number | null;
  thumbnail: string | null;
  manualSubtitleLangs: string[];
  autoCaptionLangs: string[];
  /** The uploader's own chapters, when there are any. */
  chapters: Chapter[];
};

export type Chapter = {
  title: string;
  startSeconds: number;
};

export type Transcript = {
  text: string;
  source: "captions" | "whisper";
  language: string | null;
  filePath: string | null;
  pdfPath: string | null;
  /** The timestamped document itself, so a queue can stitch several together. */
  markdown: string | null;
};

export type JobResult = {
  title: string;
  /** Where everything for this job was written. Always present. */
  folder: string;
  /** null when only the video was asked for. */
  audioPath: string | null;
  /** Present when the video was kept. */
  videoPath: string | null;
  transcript: Transcript | null;
  warnings: string[];
};

export type JobRequest = {
  url: string;
  outDir: string | null;
  /** Keep the MP3. Audio may still be fetched for a transcript and discarded. */
  wantAudio: boolean;
  wantTranscript: boolean;
  transcriptLangs: string[];
  audioQuality: AudioQuality;
  whisperModelPath: string | null;
  transcriptFormat: TranscriptFormat;
  /** Also read the text that appears on screen. */
  readScreen: boolean;
  /** Keep the video file instead of only the MP3. */
  keepVideo: boolean;
};

export type LocalFileRequest = {
  path: string;
  whisperModelPath: string | null;
  /** null lets Whisper detect the language. */
  language: string | null;
  transcriptFormat: TranscriptFormat;
  readScreen: boolean;
};

/** Which files a finished transcript leaves on disk. */
export type TranscriptFormat = "md" | "pdf" | "both";

/** "v0" is LAME VBR (~245 kbps); "320" is constant 320 kbps. */
export type AudioQuality = "v0" | "320";

export type Stage =
  | "idle"
  | "probing"
  | "downloading"
  | "converting"
  | "reading"
  | "transcribing"
  | "done";

export type ProgressEvent = {
  stage: Stage;
  percent: number | null;
  detail: string | null;
};

export function checkDependencies(): Promise<DependencyStatus[]> {
  return invoke("check_dependencies");
}

/** A ggml model found in the app's models folder, or null if there is none. */
export function defaultWhisperModel(): Promise<string | null> {
  return invoke("default_whisper_model");
}

export function probeVideo(url: string): Promise<VideoMeta> {
  return invoke("probe_video", { url });
}

export type PlaylistItem = {
  id: string;
  title: string;
  url: string;
};

export type Playlist = {
  title: string;
  items: PlaylistItem[];
};

/** What is in a playlist, without fetching any of it. */
export function probePlaylist(url: string): Promise<Playlist> {
  return invoke("probe_playlist", { url });
}

export function runJob(request: JobRequest): Promise<JobResult> {
  return invoke("run_job", { request });
}

export function transcribeFile(request: LocalFileRequest): Promise<JobResult> {
  return invoke("transcribe_file", { request });
}

/**
 * Stop the running job and delete whatever it has written so far. The job's
 * own call then rejects with "Stopped."
 */
export function cancelJob(): Promise<void> {
  return invoke("cancel_job");
}

export type CombineSection = {
  title: string;
  source: string;
  markdown: string;
};

export type CombineRequest = {
  sections: CombineSection[];
  outDir: string | null;
  transcriptFormat: TranscriptFormat;
};

export type CombineResult = {
  title: string;
  markdownPath: string | null;
  pdfPath: string | null;
};

/** One document for a whole queue, alongside the per-video files. */
export function combineTranscripts(request: CombineRequest): Promise<CombineResult> {
  return invoke("combine_transcripts", { request });
}

export type AudioInput = {
  /** The index avfoundation expects, not a position in the list. */
  index: number;
  name: string;
};

export type RecordingResult = {
  tempPath: string;
  seconds: number;
};

export function listAudioInputs(): Promise<AudioInput[]> {
  return invoke("list_audio_inputs");
}

export function setAudioInput(index: number): Promise<void> {
  return invoke("set_audio_input", { index });
}

/** Starts if idle, stops if recording. The outcome arrives as an event. */
export function toggleRecording(): Promise<void> {
  return invoke("toggle_recording");
}

/**
 * Recording is driven from two places -- the window and the menu bar icon --
 * so its state is announced rather than returned.
 */
export function onRecording(handlers: {
  started: () => void;
  stopped: (result: RecordingResult) => void;
  level: (value: number) => void;
  input: (index: number) => void;
  /** The output device the app switched to for this recording. */
  output: (name: string) => void;
  error: (message: string) => void;
}): Promise<UnlistenFn> {
  const subscriptions = [
    listen("recording://started", handlers.started),
    listen<RecordingResult>("recording://stopped", (e) => handlers.stopped(e.payload)),
    listen<number>("recording://level", (e) => handlers.level(e.payload)),
    listen<number>("recording://input", (e) => handlers.input(e.payload)),
    listen<string>("recording://output", (e) => handlers.output(e.payload)),
    listen<string>("recording://error", (e) => handlers.error(e.payload)),
  ];

  return Promise.all(subscriptions).then((offs) => () => offs.forEach((off) => off()));
}

export function saveRecording(tempPath: string, destination: string): Promise<string> {
  return invoke("save_recording", { tempPath, destination });
}

/** Opens the library folder, creating it if this is the first run. */
export function openOutputFolder(outDir: string | null): Promise<string> {
  return invoke("open_output_folder", { outDir });
}

export function revealInFileManager(path: string): Promise<void> {
  return invoke("reveal_in_file_manager", { path });
}

export function onProgress(
  handler: (event: ProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProgressEvent>("download://progress", (event) =>
    handler(event.payload),
  );
}

/** Rust errors arrive as plain strings; anything else is a genuine bug. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

export type LibraryEntry = {
  name: string;
  path: string;
  audioPath: string | null;
  videoPath: string | null;
  markdownPath: string | null;
  pdfPath: string | null;
  /** Your own notes, which are markdown too but a different document. */
  notesPath: string | null;
  /** `poster.jpg` in the folder, when it has one. */
  posterPath: string | null;
  /** Seconds since the epoch. */
  modified: number;
};

/** Turn the frosted window on or off. */
export function setGlass(glass: boolean): Promise<void> {
  return invoke("set_glass", { glass });
}

/** Every folder in the library, newest first. */
export function listLibrary(outDir: string | null): Promise<LibraryEntry[]> {
  return invoke("list_library", { outDir });
}

/**
 * The folder's picture, drawn now if it has none. Null when the folder holds
 * nothing that can be pictured.
 */
export function ensureThumbnail(path: string): Promise<string | null> {
  return invoke("ensure_thumbnail", { path });
}

export type DocumentBlock =
  | { kind: "said"; at: string; text: string }
  | { kind: "screen"; at: string; text: string }
  | { kind: "heading"; text: string }
  | { kind: "plain"; text: string };

export type DocumentChapter = {
  title: string;
  at: string | null;
  blocks: DocumentBlock[];
};

export type TranscriptDocument = {
  title: string;
  meta: string[];
  chapters: DocumentChapter[];
  /** Words that were actually said, for the line under the title. */
  words: number;
};

/** A transcript, parsed for reading inside the app. */
export function openDocument(path: string): Promise<TranscriptDocument> {
  return invoke("open_document", { path });
}

/** Every note kept for a transcript, as [chapter title, text]. */
export function readNotes(transcript: string): Promise<[string, string][]> {
  return invoke("read_notes", { transcript });
}

/**
 * Put a note against a chapter. An empty body removes it; the answer is where
 * the notes file went, or null when the last note was cleared.
 */
export function saveNote(
  transcript: string,
  title: string,
  chapter: string,
  body: string,
): Promise<string | null> {
  return invoke("save_note", { transcript, title, chapter, body });
}
