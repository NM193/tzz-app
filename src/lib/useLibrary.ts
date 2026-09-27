import { useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { ensureThumbnail, errorMessage, listLibrary, type LibraryEntry } from "./api";
import { counts as countBy, rank, type Filter } from "./library";
import { captureRects, playMoves } from "./motion";
import type { Settings } from "./settings";

export type LibraryLayout = "grid" | "list";

/**
 * Everything the Library screen knows.
 *
 * It lives here rather than in the view because the controls sit in the
 * sidebar and the results sit on the stage -- two places, one state.
 */
export function useLibrary(
  settings: Settings,
  version: number,
  box: RefObject<HTMLOListElement>,
) {
  const [entries, setEntries] = useState<LibraryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [layout, setLayout] = useState<LibraryLayout>("grid");
  const [cols, setCols] = useState(3);
  const [rows, setRows] = useState(1);
  const before = useRef<Map<string, DOMRect> | null>(null);

  useEffect(() => {
    listLibrary(settings.outDir || null)
      .then(setEntries)
      .catch((caught) => setError(errorMessage(caught)));
  }, [settings.outDir, version]);

  // Folders from before there were posters get one now, one at a time so a
  // hundred of them cannot take the machine over. Each arrives on its own.
  // Keyed on which folders exist, not on the entries themselves -- writing a
  // poster changes the entries, and that must not restart the run.
  const pathsKey = (entries ?? []).map((entry) => entry.path).join("|");
  const tried = useRef(new Set<string>());

  useEffect(() => {
    const folders = pathsKey ? pathsKey.split("|") : [];
    const missing = folders.filter((path) => !tried.current.has(path));
    if (missing.length === 0) return;

    let stopped = false;
    void (async () => {
      for (const path of missing) {
        if (stopped) return;
        tried.current.add(path);
        const poster = await ensureThumbnail(path).catch(() => null);
        if (stopped || !poster) continue;
        setEntries((all) =>
          all ? all.map((e) => (e.path === path ? { ...e, posterPath: poster } : e)) : all,
        );
      }
    })();

    return () => {
      stopped = true;
    };
  }, [pathsKey]);

  const all = useMemo(() => entries ?? [], [entries]);
  const matching = useMemo(() => rank(all, query, filter), [all, query, filter]);
  const counts = useMemo(() => countBy(all, query), [all, query]);

  // A column change re-flows the grid; the cards slide rather than jump, so
  // the positions have to be taken before the state changes.
  function changeCols(value: number) {
    const next = Math.max(2, Math.min(5, value));
    if (next === cols) return;
    if (box.current) before.current = captureRects(box.current);
    setCols(next);
  }

  useLayoutEffect(() => {
    const previous = before.current;
    before.current = null;
    if (previous && box.current) playMoves(box.current, previous, 750);
  }, [cols]);

  return {
    entries,
    error,
    setError,
    all,
    matching,
    counts,
    query,
    setQuery,
    filter,
    setFilter,
    layout,
    setLayout,
    cols,
    setCols: changeCols,
    rows,
    setRows: (value: number) => setRows(Math.max(1, Math.min(3, value))),
  };
}

export type Library = ReturnType<typeof useLibrary>;
