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
          className={`segmented__item ${value === option.value ? "segmented__item--on" : ""}`}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

type SwitchProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
};

export function Switch({ checked, onChange, label }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`switch ${checked ? "switch--on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="switch__knob" />
    </button>
  );
}

type RowProps = {
  title: string;
  hint?: ReactNode;
  children: ReactNode;
};

/** A settings line: what it is on the left, the control on the right. */
export function Row({ title, hint, children }: RowProps) {
  return (
    <div className="row">
      <div className="row__text">
        <span className="row__title">{title}</span>
        {hint && <span className="row__hint">{hint}</span>}
      </div>
      <div className="row__control">{children}</div>
    </div>
  );
}
