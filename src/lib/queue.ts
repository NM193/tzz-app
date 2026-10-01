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
  /** The label came from the paste, not from the site. */
  named?: boolean;
  result?: JobResult;
  error?: string;
};

export type Pasted = {
  url: string;
  /** A name given in the paste, for players that carry none worth having. */
  title?: string;
};

/**
 * One link per line. Blanks, duplicates and anything that is not a link are
 * dropped, so a pasted page of text cannot turn into a queue of nonsense.
 *
 * A line may name what it is: `Lesson three | https://…`. Some players report
 * no title worth keeping -- a Wistia lesson comes back as "cuku29nr8.mp4" --
 * and a course of twenty of those is unusable. Whatever collected the links
 * knows their names, so it can say them here.
 */
export function parseUrlList(text: string): Pasted[] {
  const seen = new Set<string>();

  return text
    .split("\n")
    .map((line) => {
      const at = line.indexOf("|");
      const title = at === -1 ? "" : line.slice(0, at).trim();
      const url = (at === -1 ? line : line.slice(at + 1)).trim();
      return title ? { url, title } : { url };
    })
    .filter(({ url }) => url.startsWith("http://") || url.startsWith("https://"))
    .filter(({ url }) => {
      if (seen.has(url)) return false;
      seen.add(url);
      return true;
    });
}

export function urlItem(url: string, label?: string, named?: boolean): QueueItem {
  return {
    id: crypto.randomUUID(),
    kind: "url",
    label: label ?? url,
    target: url,
    status: "waiting",
    /** The label was given rather than read, so the job should use it. */
    named,
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

export function pendingLink(url: string, title?: string): PendingLink {
  return { id: crypto.randomUUID(), url, title: title ?? null, failed: false };
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
