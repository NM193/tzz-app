import type { LibraryEntry } from "./api";

/** What a folder holds, which is the only thing there is to filter by. */
export type Filter = "all" | "audio" | "video" | "md" | "notes" | "pdf";

export const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "audio", label: "Audio" },
  { id: "video", label: "Video" },
  { id: "md", label: "Transcript" },
  { id: "notes", label: "Notes" },
  { id: "pdf", label: "PDF" },
];

export function has(entry: LibraryEntry, filter: Filter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "audio":
      return entry.audioPath !== null;
    case "video":
      return entry.videoPath !== null;
    case "md":
      return entry.markdownPath !== null;
    case "notes":
      return entry.notesPath !== null;
    case "pdf":
      return entry.pdfPath !== null;
  }
}

/** The tags a row shows: the same facts as the filters. */
export function kindsOf(entry: LibraryEntry): string[] {
  return FILTERS.filter((f) => f.id !== "all" && has(entry, f.id)).map((f) => f.label);
}

/** Lowercased, without diacritics, split on anything that is not a letter or digit. */
export function toWords(text: string): string[] {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

const anyStartsWith = (words: string[], token: string) =>
  words.some((word) => word.startsWith(token));

/**
 * Score a folder for a query; 0 keeps it out, an empty query lets everything in.
 *
 * Every word of the query has to hit something. A name word is worth more than
 * a kind word, and a name that starts with the whole query wins outright --
 * typing the first letters of a lecture should put it on top.
 */
export function score(entry: LibraryEntry, query: string): number {
  const tokens = toWords(query);
  if (tokens.length === 0) return 1;

  const name = toWords(entry.name);
  const kinds = toWords(kindsOf(entry).join(" "));

  let total = 0;
  for (const token of tokens) {
    const points = anyStartsWith(name, token) ? 60 : anyStartsWith(kinds, token) ? 25 : 0;
    if (points === 0) return 0;
    total += points;
  }
  if (name.join(" ").startsWith(tokens.join(" "))) total += 100;
  return total;
}

/** Folders that pass the filter and the query, best first, then newest. */
export function rank(entries: LibraryEntry[], query: string, filter: Filter): LibraryEntry[] {
  return entries
    .map((entry, index) => ({
      entry,
      index,
      points: has(entry, filter) ? score(entry, query) : 0,
    }))
    .filter((x) => x.points > 0)
    .sort((a, b) => b.points - a.points || a.index - b.index)
    .map((x) => x.entry);
}

/** How many folders of each kind survive the current query. */
export function counts(entries: LibraryEntry[], query: string): Record<Filter, number> {
  const out = { all: 0, audio: 0, video: 0, md: 0, pdf: 0 } as Record<Filter, number>;
  for (const entry of entries) {
    if (score(entry, query) === 0) continue;
    for (const filter of FILTERS) {
      if (has(entry, filter.id)) out[filter.id] += 1;
    }
  }
  return out;
}

export type Part = { text: string; hit: boolean };

/** Split a name so the matched beginnings of words can be marked. */
export function highlight(text: string, query: string): Part[] {
  const tokens = toWords(query).sort((a, b) => b.length - a.length);
  if (tokens.length === 0) return [{ text, hit: false }];

  const parts: Part[] = [];
  const push = (value: string, hit: boolean) => {
    if (!value) return;
    const last = parts[parts.length - 1];
    if (last && last.hit === hit) last.text += value;
    else parts.push({ text: value, hit });
  };

  for (const segment of text.split(/([\p{L}\p{N}]+)/u)) {
    const word = toWords(segment)[0];
    const token = word ? tokens.find((t) => word.startsWith(t)) : undefined;
    if (!token) {
      push(segment, false);
      continue;
    }
    push(segment.slice(0, token.length), true);
    push(segment.slice(token.length), false);
  }
  return parts;
}
