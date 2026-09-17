import { useEffect, useRef, useState, type ReactNode } from "react";
import { CheckIcon, ChevronIcon } from "./Icons";

export type Option<T extends string> = {
  value: T;
  label: string;
  hint?: string;
};

type Props<T extends string> = {
  label: string;
  icon?: ReactNode;
  /** What the closed control shows. */
  summary: string;
  options: Option<T>[];
  /** Ticked rows. One entry for a single choice, several for toggles. */
  selected: T[];
  onPick: (value: T) => void;
  /** Keep the menu open after a pick, for toggles. */
  multiple?: boolean;
  disabled?: boolean;
};

/**
 * A select drawn by the app, not the OS.
 *
 * WKWebView's native menu cannot be styled and would sit on the frosted
 * window like a sticker. This one is a plain popover, so it can also hold
 * toggles, which a select cannot.
 */
export function Dropdown<T extends string>({
  label,
  icon,
  summary,
  options,
  selected,
  onPick,
  multiple = false,
  disabled = false,
}: Props<T>) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="dropdown" ref={root}>
      <span className="dropdown__label">{label}</span>
      <button
        type="button"
        className={`dropdown__control ${open ? "dropdown__control--open" : ""}`}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        {icon && <span className="dropdown__icon">{icon}</span>}
        <span className="dropdown__summary">{summary}</span>
        <ChevronIcon size={16} className="dropdown__chevron" />
      </button>

      {open && (
        <ul className="dropdown__menu" role="listbox" aria-multiselectable={multiple}>
          {options.map((option) => {
            const on = selected.includes(option.value);
            return (
              <li key={option.value}>
                <button
                  type="button"
                  role="option"
                  aria-selected={on}
                  className={`dropdown__option ${on ? "dropdown__option--on" : ""}`}
                  onClick={() => {
                    onPick(option.value);
                    if (!multiple) setOpen(false);
                  }}
                >
                  <span className="dropdown__tick">{on && <CheckIcon size={14} />}</span>
                  <span className="dropdown__text">
                    {option.label}
                    {option.hint && <em>{option.hint}</em>}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
