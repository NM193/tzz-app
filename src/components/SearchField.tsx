import { useEffect, useRef, useState } from "react";
import { CloseIcon } from "./Icons";

const EXAMPLES = ["Try “webflow”", "Try “lecture”", "Try “design”", "Try “pdf”"];

type Props = {
  value: string;
  onChange: (value: string) => void;
};

/**
 * While it is empty and not focused, the field types out examples. It is the
 * only thing on the screen that moves on its own, so it also says, quietly,
 * that the library is searchable at all.
 */
export function SearchField({ value, onChange }: Props) {
  const input = useRef<HTMLInputElement>(null);
  const ghost = useRef<HTMLSpanElement>(null);
  const [focused, setFocused] = useState(false);
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const idle = !still && !focused && !value;

  // "/" from anywhere focuses the field -- unless something else is taking text.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable='true']")) return;
      event.preventDefault();
      input.current?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const el = ghost.current;
    if (!el || !idle) return;

    let word = 0;
    let length = 0;
    let step = 1;
    let timer = 0;

    const tick = () => {
      const text = EXAMPLES[word];
      el.textContent = text.slice(0, length);
      if (step > 0 && length === text.length) {
        step = -1;
        timer = window.setTimeout(tick, 1500);
        return;
      }
      if (step < 0 && length === 0) {
        step = 1;
        word = (word + 1) % EXAMPLES.length;
        timer = window.setTimeout(tick, 300);
        return;
      }
      length += step;
      timer = window.setTimeout(tick, step > 0 ? 65 : 28);
    };
    tick();

    return () => {
      window.clearTimeout(timer);
      el.textContent = "";
    };
  }, [idle]);

  return (
    <label
      className="search"
      data-focus={focused ? "" : undefined}
      data-has={value ? "" : undefined}
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="11" cy="11" r="7" />
        <path d="m20 20-3.5-3.5" />
      </svg>

      <input
        ref={input}
        type="search"
        value={value}
        aria-label="Search the library"
        placeholder={still ? "Search the library" : ""}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
          if (event.key === "Escape") {
            onChange("");
            event.currentTarget.blur();
          }
        }}
      />

      <span ref={ghost} className="search__ghost" aria-hidden="true" />
      <kbd className="search__key" aria-hidden="true">
        /
      </kbd>
      <button
        type="button"
        className="search__clear"
        aria-label="Clear search"
        tabIndex={value ? 0 : -1}
        onClick={() => {
          onChange("");
          input.current?.focus();
        }}
      >
        <CloseIcon size={12} />
      </button>
    </label>
  );
}
