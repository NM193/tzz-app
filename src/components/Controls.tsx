import type { ReactNode } from "react";

type SegmentedProps<T extends string> = {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  label: string;
};

/** A row of choices where exactly one is on. */
export function Segmented<T extends string>({ options, value, onChange, label }: SegmentedProps<T>) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      className="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
    >
      <i />
    </button>
  );
}

/** A settings line: what it is on the left, the control on the right. */
export function Setting({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="setting">
      <div className="setting__text">
        <b>{title}</b>
        {hint && <span>{hint}</span>}
      </div>
      <div className="setting__control">{children}</div>
    </div>
  );
}

/** One line of the tick lists that replaced the option menus. */
export function Check({
  on,
  onToggle,
  label,
  hint,
  disabled,
}: {
  on: boolean;
  onToggle: () => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={on}
      data-on={on ? "" : undefined}
      disabled={disabled}
      onClick={onToggle}
    >
      <i className="tick" />
      {label}
      {hint && <em>{hint}</em>}
    </button>
  );
}
