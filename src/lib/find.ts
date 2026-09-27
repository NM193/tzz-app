/**
 * Finding a word inside a transcript.
 *
 * Plain substring matching, not the library's word-prefix scoring: in a
 * document you are looking for the letters you typed, wherever they sit.
 *
 * Accents are ignored in both directions, so "sta" finds "šta" and vice
 * versa. That cannot be done by folding the whole string and matching on it --
 * folding changes the length (a precomposed "š" is one character, a decomposed
 * one is two) and the positions would no longer line up. So the folded text
 * carries a map back to where each of its characters came from.
 */

import type { TranscriptDocument } from "./api";

/** Letters that have no combining form to strip. */
const SPECIAL: Record<string, string> = {
  đ: "d",
  Đ: "d",
  ð: "d",
  ł: "l",
  Ł: "l",
  ß: "ss",
};

function foldChar(ch: string): string {
  return (
    SPECIAL[ch] ??
    ch
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
  );
}

/** The text with accents and case removed, and where each character came from. */
function fold(text: string): { folded: string; from: number[] } {
  let folded = "";
  const from: number[] = [];
  for (let i = 0; i < text.length; i += 1) {
    const piece = foldChar(text[i]);
    for (let k = 0; k < piece.length; k += 1) from.push(i);
    folded += piece;
  }
  return { folded, from };
}

export type Part = { text: string; hit: boolean };

/** Split a line so the matches can be marked, in the original spelling. */
export function parts(text: string, needle: string): Part[] {
  const query = fold(needle).folded.trim();
  if (query.length === 0) return [{ text, hit: false }];

  const { folded, from } = fold(text);
  const out: Part[] = [];
  let read = 0;
  let at = folded.indexOf(query);

  while (at !== -1) {
    const start = from[at];
    // One past the last character the match covers, in the original.
    const end = at + query.length < from.length ? from[at + query.length] : text.length;
    if (start > read) out.push({ text: text.slice(read, start), hit: false });
    out.push({ text: text.slice(start, end), hit: true });
    read = end;
    at = folded.indexOf(query, at + query.length);
  }

  if (read < text.length) out.push({ text: text.slice(read), hit: false });
  return out.length > 0 ? out : [{ text, hit: false }];
}

/** How many times the query appears in this line. */
export function count(text: string, needle: string): number {
  const query = fold(needle).folded.trim();
  if (query.length === 0) return 0;

  const { folded } = fold(text);
  let found = 0;
  let at = folded.indexOf(query);
  while (at !== -1) {
    found += 1;
    at = folded.indexOf(query, at + query.length);
  }
  return found;
}

export type Scan = {
  total: number;
  /** How many matches each chapter holds, for the contents. */
  perChapter: number[];
  /** The global number of the first match in each block, chapter by chapter. */
  offsets: number[][];
};

/**
 * Number every match in the document in reading order.
 *
 * The reader marks each one with its number so that "next" is a scroll to an
 * element rather than a search repeated. The walk must follow exactly what is
 * rendered -- so hidden screen lines are not counted either.
 */
export function scan(
  doc: TranscriptDocument | null,
  needle: string,
  withScreens: boolean,
): Scan {
  const empty: Scan = { total: 0, perChapter: [], offsets: [] };
  if (!doc || needle.trim().length === 0) {
    return doc ? { ...empty, perChapter: doc.chapters.map(() => 0), offsets: doc.chapters.map((c) => c.blocks.map(() => 0)) } : empty;
  }

  let total = 0;
  const perChapter: number[] = [];
  const offsets: number[][] = [];

  for (const chapter of doc.chapters) {
    const before = total;
    const here: number[] = [];
    for (const block of chapter.blocks) {
      here.push(total);
      if (block.kind === "screen" && !withScreens) continue;
      total += count(block.text, needle);
    }
    offsets.push(here);
    perChapter.push(total - before);
  }

  return { total, perChapter, offsets };
}
