import type { AudioQuality, TranscriptFormat } from "./api";

export type Theme = "dark" | "light";

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
  theme: Theme;
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
  theme: "dark",
};

// The key is versioned; a shape change bumps it rather than migrating.
const STORAGE_KEY = "yt-mp3.settings.v2";

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
