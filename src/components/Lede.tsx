import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { EASE, OUT, stillPreferred } from "../lib/motion";

type Props = {
  text: string;
  /** Every sentence this line can hold. The tallest one becomes its height. */
  measure: string[];
};

/**
 * The line under a heading, which must never move what sits below it.
 *
 * Its height is reserved before anything animates: a hidden clone of the
 * paragraph, at the same width and font, is measured against each sentence it
 * could hold. Only a long search can still need an extra line, and then the
 * height itself animates rather than jumping.
 */
export function Lede({ text, measure }: Props) {
  const ref = useRef<HTMLParagraphElement>(null);
  const [shown, setShown] = useState(text);
  const heightBefore = useRef<number | null>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;

    const fit = () => {
      if (!el.isConnected) return;
      const probe = el.cloneNode() as HTMLElement;
      probe.style.cssText =
        `position:absolute;visibility:hidden;min-height:0;` +
        `width:${el.getBoundingClientRect().width}px`;
      el.after(probe);

      let tallest = 0;
      for (const sentence of measure) {
        probe.textContent = sentence;
        tallest = Math.max(tallest, probe.offsetHeight);
      }
      probe.remove();
      el.style.minHeight = `${tallest}px`;
    };

    fit();
    const observer = new ResizeObserver(fit);
    if (el.parentElement) observer.observe(el.parentElement);
    // The serif is bundled, but it still arrives after the first layout.
    void document.fonts?.ready.then(fit);
    return () => observer.disconnect();
  }, [measure]);

  useEffect(() => {
    if (text === shown) return;
    const el = ref.current;
    const still = stillPreferred();

    const out =
      el && !still
        ? el.animate(
            [
              { opacity: 1, transform: "none" },
              { opacity: 0, transform: "translateY(-0.375rem)" },
            ],
            { duration: 160, easing: OUT, fill: "forwards" },
          )
        : null;

    const timer = window.setTimeout(
      () => {
        heightBefore.current = el ? el.offsetHeight : null;
        setShown(text);
      },
      out ? 160 : 0,
    );

    return () => {
      window.clearTimeout(timer);
      out?.cancel();
    };
  }, [text, shown]);

  useLayoutEffect(() => {
    const el = ref.current;
    const from = heightBefore.current;
    heightBefore.current = null;
    if (!el || from === null) return;

    el.getAnimations().forEach((animation) => animation.cancel());
    if (stillPreferred()) return;

    const to = el.offsetHeight;
    if (from !== to) {
      el.animate([{ height: `${from}px` }, { height: `${to}px` }], {
        duration: 420,
        easing: EASE,
      });
    }
    el.animate(
      [
        { opacity: 0, transform: "translateY(0.625rem)" },
        { opacity: 1, transform: "none" },
      ],
      { duration: 480, easing: EASE },
    );
  }, [shown]);

  return (
    <p className="lede" ref={ref}>
      {shown}
    </p>
  );
}
