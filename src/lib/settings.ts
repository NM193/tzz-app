import type { AudioQuality, TranscriptFormat } from "./api";

/** The window either shows the desktop through it, or it does not. */
export type Surface = "solid" | "glass" | "aurora";
export type Accent = "amber" | "green";
/** The veil over the blur, on glass. */
export type Tint = "warm" | "cool" | "ink" | "clear";
/** The sky the app paints for itself, on aurora. */
export type Aurora = "dusk" | "ember" | "moss";

export type Settings = {
  outDir: string;
  wantAudio: boolean;
  wantTranscript: boolean;
  keepVideo: boolean;
  transcriptLangs: string[];
  audioQuality: AudioQuality;
  whisperModelPath: string;
  transcriptFormat: TranscriptFormat;
  combineQueue: boolean;
  readScreen: boolean;
  surface: Surface;
  accent: Accent;
  tint: Tint;
  aurora: Aurora;
};

export const DEFAULT_SETTINGS: Settings = {
  outDir: "",
  wantAudio: true,
  wantTranscript: true,
  keepVideo: false,
  transcriptLangs: ["sr", "en"],
  audioQuality: "v0",
  whisperModelPath: "",
  transcriptFormat: "md",
  combineQueue: true,
  readScreen: false,
  surface: "glass",
  accent: "amber",
  tint: "warm",
  aurora: "dusk",
};

// The key is versioned; a shape change bumps it rather than migrating.
const STORAGE_KEY = "yt-mp3.settings.v3";

export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } : DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: Settings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // A full or blocked store only costs remembering; never the job.
  }
}

/** Where output goes, as the user reads it. */
export function libraryLabel(outDir: string): string {
  if (!outDir) return "Tzz Library";
  return outDir.split("/").filter(Boolean).pop() ?? outDir;
}
