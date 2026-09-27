import { useLayoutEffect, useRef } from "react";
import { EASE, stillPreferred } from "../lib/motion";

/**
 * The number in brackets. The old one leaves, the new one arrives -- upward
 * when the count grows -- and the width animates so the bracket never jumps.
 */
export function CountRoll({ value }: { value: number }) {
  const box = useRef<HTMLSpanElement>(null);
  const previous = useRef(value);

  useLayoutEffect(() => {
    const el = box.current;
    const from = previous.current;
    previous.current = value;
    if (!el || from === value || stillPreferred()) return;

    const now = el.querySelector<HTMLElement>("[data-now]");
    if (!now) return;

    el.querySelectorAll("[data-old]").forEach((node) => node.remove());
    const old = document.createElement("span");
    old.dataset.old = "";
    old.setAttribute("aria-hidden", "true");
    old.textContent = String(from);
    el.append(old);

    const pad = el.offsetWidth - now.offsetWidth;
    const up = value > from ? 1 : -1;

    el.animate([{ width: `${old.offsetWidth + pad}px` }, { width: `${el.offsetWidth}px` }], {
      duration: 520,
      easing: EASE,
    });
    old.animate(
      [
        { transform: "none", opacity: 1 },
        { transform: `translateY(${-up * 90}%)`, opacity: 0 },
      ],
      { duration: 420, easing: EASE, fill: "forwards" },
    ).onfinish = () => old.remove();
    now.animate(
      [
        { transform: `translateY(${up * 90}%)`, opacity: 0 },
        { transform: "none", opacity: 1 },
      ],
      { duration: 520, easing: EASE, fill: "backwards" },
    );
  }, [value]);

  return (
    <span className="odo" ref={box}>
      <span data-now>{value}</span>
    </span>
  );
}
