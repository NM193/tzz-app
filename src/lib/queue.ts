import type { JobResult } from "./api";

export type QueueStatus = "waiting" | "running" | "done" | "failed";

export type QueueItem = {
  id: string;
  /** Decides which command runs: a YouTube job or a local transcription. */
  kind: "url" | "file";
  /** The link, or the file's name. What the row shows. */
  label: string;
  /** The link, or the absolute file path. What the command receives. */
  target: string;
  status: QueueStatus;
  result?: JobResult;
  error?: string;
};

/**
 * One link per line. Blanks, duplicates and anything that is not a link are
 * dropped, so a pasted page of text cannot turn into a queue of nonsense.
 */
export function parseUrlList(text: string): string[] {
  const seen = new Set<string>();
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("http://") || line.startsWith("https://"))
    .filter((line) => {
      if (seen.has(line)) return false;
      seen.add(line);
      return true;
    });
}

export function urlItem(url: string, label?: string): QueueItem {
  return {
    id: crypto.randomUUID(),
    kind: "url",
    label: label ?? url,
    target: url,
    status: "waiting",
  };
}

/**
 * A link sitting in the input field, before the queue starts.
 *
 * The title is fetched as soon as it is pasted so ten lectures can be told
 * apart while choosing, not only once they start downloading.
 */
export type PendingLink = {
  id: string;
  url: string;
  /** null while the title is still being read. */
  title: string | null;
  /** The probe failed. Still queueable -- the job may well work anyway. */
  failed: boolean;
  /** A playlist waiting to be unpacked into the videos it holds. */
  playlist?: boolean;
};

export function pendingLink(url: string): PendingLink {
  return { id: crypto.randomUUID(), url, title: null, failed: false };
}

/** Mirrors PLAYLIST_LIMIT in src-tauri/src/services/ytdlp.rs. */
export const PLAYLIST_LIMIT = 100;

/**
 * A link to a whole playlist, rather than one video.
 *
 * Only `/playlist?list=...` counts. A watch link often carries a `list` as
 * well -- that is the video you clicked from inside a playlist, and expanding
 * it into forty pills is not what anyone meant by pasting it.
 */
export function isPlaylist(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (!/(^|\.)(youtube\.com|youtu\.be)$/.test(parsed.host)) return false;
    return parsed.pathname === "/playlist" && parsed.searchParams.has("list");
  } catch {
    return false;
  }
}

/** A link whose title is already known, so nothing needs to be read for it. */
export function knownLink(url: string, title: string): PendingLink {
  return { id: crypto.randomUUID(), url, title, failed: false };
}

/** youtube.com/watch?v=abc -> youtu.be/abc, for a pill that fits. */
export function shortenUrl(url: string): string {
  try {
    const parsed = new URL(url);
    const id = parsed.searchParams.get("v");
    if (id) return `youtu.be/${id}`;
    return parsed.host.replace(/^www\./, "") + parsed.pathname;
  } catch {
    return url;
  }
}

export function fileItem(path: string): QueueItem {
  return {
    id: crypto.randomUUID(),
    kind: "file",
    label: path.split("/").pop() ?? path,
    target: path,
    status: "waiting",
  };
}

/** "8 done, 2 failed" -- or null while nothing has finished. */
export function summarise(items: QueueItem[]): string | null {
  const done = items.filter((i) => i.status === "done").length;
  const failed = items.filter((i) => i.status === "failed").length;
  if (done + failed === 0) return null;
  return failed === 0 ? `${done} done` : `${done} done, ${failed} failed`;
}
