import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { captureRects, playMoves, stillPreferred } from "./motion";

type Enter = (el: HTMLElement, index: number) => void;
type Exit = (el: HTMLElement) => Animation;

/** How long the leaving elements are given before the list is rewritten. */
const EXIT_MS = 230;

/**
 * A list that lags behind its own data for exactly as long as the exit takes.
 *
 * React drops an element the moment it leaves the array, so an exit animation
 * has nothing to play on. The order is: play the exits, measure where
 * everything sits, write the new list, then in a layout effect slide the
 * survivors (FLIP) and give the newcomers their entrance.
 *
 * `resetKey` changes when the list is replaced wholesale rather than filtered
 * -- switching between grid and list -- and everything simply enters again.
 */
export function useListTransitions(
  ref: RefObject<HTMLElement | null>,
  target: string[],
  enter: Enter,
  exit: Exit,
  resetKey: string,
): string[] {
  const [shown, setShown] = useState(target);
  const before = useRef<Map<string, DOMRect> | null>(null);

  const key = target.join("|");
  const shownKey = shown.join("|");

  useEffect(() => {
    if (key === shownKey) return;

    const next = key ? key.split("|") : [];
    const box = ref.current;
    const keep = new Set(next);
    const still = stillPreferred();

    const leaving =
      box && !still
        ? [...box.querySelectorAll<HTMLElement>("[data-id]")].filter(
            (el) => !keep.has(el.dataset.id ?? ""),
          )
        : [];
    const exits = leaving.map(exit);

    const timer = window.setTimeout(
      () => {
        before.current = box ? captureRects(box) : null;
        setShown(next);
      },
      leaving.length > 0 ? EXIT_MS : 0,
    );

    return () => {
      window.clearTimeout(timer);
      exits.forEach((animation) => animation.cancel());
    };
  }, [key, shownKey, ref, exit]);

  useLayoutEffect(() => {
    const box = ref.current;
    const previous = before.current;
    before.current = null;
    if (!box || !previous) return;
    playMoves(box, previous, 650).forEach(enter);
  }, [shownKey, ref, enter]);

  // Grid and list are different elements; whichever appears starts from zero.
  useLayoutEffect(() => {
    const box = ref.current;
    if (!box) return;
    box.querySelectorAll<HTMLElement>("[data-id]").forEach(enter);
  }, [resetKey, ref, enter]);

  return shown;
}
