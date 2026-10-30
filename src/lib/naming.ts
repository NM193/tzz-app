/**
 * The dialog that asks about names the library already has, as plain
 * functions: which jobs it asks about, the batch as it would run with the
 * answers so far, which rows cannot start, and what each row's line says.
 *
 * Types are all it imports, so Node runs the tests for this file as is.
 */
import type { Candidate, Clash, Holds } from "./api";
import type { QueueItem } from "./queue";

export type Choice = "rename" | "replace" | "skip";

export type NameRow = {
  key: string;
  choice: Choice;
  /** What the New name field holds, suggested or typed. */
  name: string;
};

/** A job as the library check sees it. A link is checked only with its id. */
export function toCandidates(items: QueueItem[]): Candidate[] {
  return items.map((item) =>
    item.kind === "file"
      ? { key: item.id, path: item.target, name: item.name ?? null }
      : { key: item.id, title: item.named ? item.label : null, videoId: item.videoId ?? null },
  );
}

/** The rows: every job the first check found taken, with New name chosen. */
export function rowsFrom(clashes: Clash[]): NameRow[] {
  return clashes
    .filter((clash) => clash.inLibrary || clash.sameAs !== null)
    .map((clash) => ({ key: clash.key, choice: "rename", name: clash.suggestion ?? "" }));
}

/**
 * The batch as it would run with the answers so far: what the dialog checks
 * again on every change, and what it hands to the queue at the end.
 */
export function asRun(items: QueueItem[], rows: NameRow[]): QueueItem[] {
  const byKey = new Map(rows.map((row) => [row.key, row]));
  return items.flatMap((item): QueueItem[] => {
    const row = byKey.get(item.id);
    if (!row) return [item];
    if (row.choice === "skip") return [];
    if (row.choice === "replace") return [{ ...item, replace: true }];
    const name = row.name.trim();
    // A link's new name travels the way a pasted name does: as its title.
    return [item.kind === "file" ? { ...item, name } : { ...item, label: name, named: true }];
  });
}

/**
 * Which rows cannot start as they are. Only a row's name can change between
 * checks, so a clash is always a row's doing and is pinned on one: on the
 * row itself when the other job has none, and between two rows on the one
 * just edited, or else the later one. A Replace row is never refused for
 * the folder it means to replace.
 */
export function refused(rows: NameRow[], clashes: Clash[], edited: string | null): Set<string> {
  const choice = new Map(rows.map((row) => [row.key, row.choice]));
  const isRow = (key: string) => choice.has(key) && choice.get(key) !== "skip";
  const out = new Set<string>();
  for (const clash of clashes) {
    const row = isRow(clash.key);
    if (row && choice.get(clash.key) === "rename" && (!clash.usable || clash.inLibrary)) {
      out.add(clash.key);
    }
    const other = clash.sameAs;
    if (other === null) continue;
    if (row && isRow(other)) out.add(edited === other ? other : clash.key);
    else if (row) out.add(clash.key);
    else if (isRow(other)) out.add(other);
  }
  return out;
}

/** Why a refused row cannot start. */
export function refusalLine(row: NameRow, clash: Clash | undefined): string {
  if (row.name.trim() === "") return "Needs a name";
  if (clash && !clash.usable) return "That name cannot be a folder";
  return "That name is taken too";
}

function listed(parts: string[]): string {
  if (parts.length < 2) return parts[0] ?? "";
  return `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
}

/** What Replace takes away, built from what the folder holds. */
export function replaceLine(holds: Holds): string {
  const goes = [
    holds.audio && "MP3",
    holds.transcript && "transcript",
    holds.video && "video",
    holds.screens && "screen text",
  ].filter((part): part is string => typeof part === "string");
  const line = goes.length === 0 ? "Nothing goes" : `${listed(goes)} ${goes.length === 1 ? "goes" : "go"}`;
  return holds.notes ? `${line} · Notes stay` : line;
}

/** "~/Downloads/Week 1": the folder a file comes from, home shortened. */
export function fromFolder(path: string): string {
  const parent = path.slice(0, path.lastIndexOf("/")) || "/";
  return parent.replace(/^\/Users\/[^/]+/, "~");
}

const DAY = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" });

/**
 * What a row says when nothing is wrong with it. A file says where it comes
 * from, since two rows can carry one file name; a link says when its folder
 * was finished. Then what the folder holds, or that another job has it.
 */
export function stateLine(item: QueueItem, clash: Clash): string {
  const parts: string[] = [];
  if (item.kind === "file") parts.push(`From ${fromFolder(item.target)}`);
  else if (clash.inLibrary) parts.push(`Finished ${DAY.format(new Date(clash.holds.modified * 1000))}`);
  if (clash.sameAs !== null) {
    parts.push("Same name as another in this start");
  } else {
    if (clash.holds.audio) parts.push("MP3");
    if (clash.holds.transcript) parts.push("Transcript");
    if (clash.holds.video) parts.push("Video");
    if (clash.holds.notes) parts.push("Notes");
  }
  return parts.join(" · ");
}
