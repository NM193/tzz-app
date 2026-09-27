/**
 * The page's one gesture: things arrive from below, and rules draw from the
 * left. Values come from docs/design-reference-work.md.
 *
 * These drive the DOM directly. The global stylesheet shortens CSS animations
 * under `prefers-reduced-motion`, but not these, so every function checks.
 */

export const EASE = "cubic-bezier(.2,.8,.2,1)";
export const WIPE = "cubic-bezier(.7,0,.2,1)";
export const OUT = "cubic-bezier(.4,0,1,1)";

export function stillPreferred(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** A row's underline draws in from the left. */
export function drawRule(el: HTMLElement, index: number) {
  if (stillPreferred()) return;
  el.animate([{ transform: "scaleX(0)" }, { transform: "scaleX(1)" }], {
    pseudoElement: "::after",
    duration: 900,
    delay: index * 45,
    easing: WIPE,
    fill: "backwards",
  });
}

/** A list row: the rule draws, and the row's parts rise out of the row. */
export function rowEnter(el: HTMLElement, index: number) {
  if (stillPreferred()) return;
  drawRule(el, index);
  el.querySelectorAll<HTMLElement>("[data-part]").forEach((part, k) =>
    part.animate([{ transform: "translateY(3rem)" }, { transform: "none" }], {
      duration: 850,
      delay: index * 45 + 60 + k * 35,
      easing: EASE,
      fill: "backwards",
    }),
  );
}

/** A card: the cover opens bottom to top, then the text rises under it. */
export function cardEnter(el: HTMLElement, index: number) {
  if (stillPreferred()) return;
  const delay = index * 60;
  el.querySelector<HTMLElement>("[data-cover]")?.animate(
    [
      { clipPath: "inset(100% 0 0 0 round .25rem)" },
      { clipPath: "inset(0 0 0 0 round .25rem)" },
    ],
    { duration: 900, delay, easing: WIPE, fill: "backwards" },
  );
  el.querySelectorAll<HTMLElement>("[data-text]").forEach((t, k) =>
    t.animate(
      [
        { opacity: 0, transform: "translateY(.625rem)" },
        { opacity: 1, transform: "none" },
      ],
      { duration: 600, delay: delay + 350 + k * 80, easing: EASE, fill: "backwards" },
    ),
  );
}

/** Where every `[data-id]` child sits right now -- the "before" of a FLIP. */
export function captureRects(box: HTMLElement): Map<string, DOMRect> {
  return new Map(
    [...box.querySelectorAll<HTMLElement>("[data-id]")].map((el) => [
      el.dataset.id ?? "",
      el.getBoundingClientRect(),
    ]),
  );
}

/**
 * Slide every element that moved to its new place, scaling it if it also
 * changed width. Returns the elements that were not there before -- those need
 * an entrance rather than a move.
 */
export function playMoves(
  box: HTMLElement,
  before: Map<string, DOMRect>,
  duration: number,
): HTMLElement[] {
  const added: HTMLElement[] = [];
  const still = stillPreferred();

  box.querySelectorAll<HTMLElement>("[data-id]").forEach((el) => {
    const a = before.get(el.dataset.id ?? "");
    if (!a) {
      added.push(el);
      return;
    }
    if (still) return;
    const b = el.getBoundingClientRect();
    const dx = a.left - b.left;
    const dy = a.top - b.top;
    if (!dx && !dy && a.width === b.width) return;
    el.animate(
      [
        {
          transformOrigin: "0 0",
          transform: `translate(${dx}px, ${dy}px) scale(${a.width / b.width})`,
        },
        { transformOrigin: "0 0", transform: "none" },
      ],
      { duration, easing: EASE },
    );
  });

  return added;
}

/** A card the filter has taken away: the cover closes downward. */
export function cardExit(el: HTMLElement): Animation {
  return el.animate(
    [{ clipPath: "inset(0 0 0 0)" }, { clipPath: "inset(0 0 100% 0)" }],
    { duration: 220, easing: OUT, fill: "forwards" },
  );
}

/** A row the filter has taken away: it fades and slips left. */
export function rowExit(el: HTMLElement): Animation {
  return el.animate(
    [
      { opacity: 1, transform: "none" },
      { opacity: 0, transform: "translateX(-0.75rem)" },
    ],
    { duration: 200, easing: OUT, fill: "forwards" },
  );
}
