import { useEffect, useRef } from "react";
import { CloseIcon } from "./Icons";

type Props = {
  value: string;
  onChange: (value: string) => void;
  total: number;
  hit: number;
  onStep: (by: number) => void;
};

/**
 * Find a word in the open document.
 *
 * Enter moves to the next match and Shift+Enter to the previous, which is what
 * every other find field on the machine does. Cmd+F reaches it from anywhere
 * in the reader.
 */
export function FindField({ value, onChange, total, hit, onStep }: Props) {
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "f" || !(event.metaKey || event.ctrlKey)) return;
      event.preventDefault();
      input.current?.focus();
      input.current?.select();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const nothing = value.trim().length > 0 && total === 0;

  return (
    <div className="find" data-empty={nothing ? "" : undefined}>
      <input
        ref={input}
        type="search"
        value={value}
        placeholder="Find in this document"
        aria-label="Find in this document"
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            onStep(event.shiftKey ? -1 : 1);
          }
          if (event.key === "Escape") {
            onChange("");
            event.currentTarget.blur();
          }
        }}
      />

      {value.trim().length > 0 && (
        <span className="find__count">{total === 0 ? "none" : `${hit + 1}/${total}`}</span>
      )}

      {total > 1 && (
        <span className="find__step">
          <button type="button" aria-label="Previous match" onClick={() => onStep(-1)}>
            ‹
          </button>
          <button type="button" aria-label="Next match" onClick={() => onStep(1)}>
            ›
          </button>
        </span>
      )}

      {value.length > 0 && (
        <button
          type="button"
          className="find__clear"
          aria-label="Clear"
          onClick={() => {
            onChange("");
            input.current?.focus();
          }}
        >
          <CloseIcon size={11} />
        </button>
      )}
    </div>
  );
}
