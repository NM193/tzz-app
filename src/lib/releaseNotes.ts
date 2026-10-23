/**
 * What changed, read from the update manifest.
 *
 * `scripts/release.sh` writes a `changes` list beside the manifest's plain
 * `notes`: the new version and the nine before it, newest first. The updater
 * ignores fields it does not know and hands the whole manifest over as
 * `rawJson`, so the notice can show everything since the version you have,
 * not only the newest.
 *
 * Plain values in, nothing imported: Node runs the test for this file as is.
 */

export type VersionNotes = {
  version: string;
  new: string[];
  improved: string[];
  fixed: string[];
};

/** The sections, in the order they are shown. */
export const SECTIONS = [
  ["new", "New"],
  ["improved", "Improved"],
  ["fixed", "Fixed"],
] as const;

const NUMBERS = /^\d+\.\d+\.\d+$/;

/** The versions in `manifest` newer than `current`, newest first. */
export function notesSince(manifest: unknown, current: string): VersionNotes[] {
  const changes = isRecord(manifest) ? manifest.changes : undefined;
  if (!Array.isArray(changes)) return [];
  return changes
    .map(readEntry)
    .filter(
      (entry): entry is VersionNotes =>
        entry !== null && compareVersions(entry.version, current) > 0,
    )
    .sort((a, b) => compareVersions(b.version, a.version));
}

/** Below, at or above zero as `a` is older than, the same as or newer than `b`. */
export function compareVersions(a: string, b: string): number {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

function readEntry(raw: unknown): VersionNotes | null {
  if (!isRecord(raw) || typeof raw.version !== "string" || !NUMBERS.test(raw.version)) {
    return null;
  }
  const entry = {
    version: raw.version,
    new: lines(raw.new),
    improved: lines(raw.improved),
    fixed: lines(raw.fixed),
  };
  return entry.new.length + entry.improved.length + entry.fixed.length > 0 ? entry : null;
}

function lines(raw: unknown): string[] {
  return Array.isArray(raw)
    ? raw.filter((line): line is string => typeof line === "string" && line.trim() !== "")
    : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
