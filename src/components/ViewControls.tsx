import { useLayoutEffect, useRef, type CSSProperties, type ReactNode } from "react";
import { EASE, stillPreferred } from "../lib/motion";

export type LibraryView = "grid" | "list";

/** Twelve tiles in N columns; changing N slides them to their new places. */
function MiniGrid({ cols }: { cols: number }) {
  const box = useRef<HTMLSpanElement>(null);
  const last = useRef<{ x: number; y: number; w: number }[] | null>(null);

  useLayoutEffect(() => {
    const tiles = [...(box.current?.children ?? [])] as HTMLElement[];
    const now = tiles.map((t) => ({ x: t.offsetLeft, y: t.offsetTop, w: t.offsetWidth }));
    const before = last.current;
    last.current = now;
    if (!before || stillPreferred()) return;

    tiles.forEach((tile, index) => {
      const a = before[index];
      const b = now[index];
      if (!a || !b.w || (a.x === b.x && a.y === b.y && a.w === b.w)) return;
      tile.animate(
        [
          {
            transformOrigin: "0 0",
            transform: `translate(${a.x - b.x}px, ${a.y - b.y}px) scale(${a.w / b.w})`,
          },
          { transformOrigin: "0 0", transform: "none" },
        ],
        { duration: 600, easing: EASE },
      );
    });
  }, [cols]);

  return (
    <span className="mini__grid" ref={box} style={{ "--n": cols } as CSSProperties}>
      {Array.from({ length: 12 }, (_, i) => (
        <span key={i} />
      ))}
    </span>
  );
}

function MiniList({ rows }: { rows: number }) {
  return (
    <span className="mini__list" data-rows={rows}>
      {Array.from({ length: 8 }, (_, i) => (
        <span key={i} />
      ))}
    </span>
  );
}

/** Minus, a picture of the layout, plus. The picture is the label. */
function Stepper({
  className,
  label,
  value,
  min,
  max,
  less,
  more,
  inactive,
  onChange,
  children,
}: {
  className: string;
  label: string;
  value: number;
  min: number;
  max: number;
  less: string;
  more: string;
  inactive: boolean;
  onChange: (value: number) => void;
  children: ReactNode;
}) {
  return (
    <div className={`stepper ${className}`} role="group" aria-label={label} inert={inactive}>
      <button
        type="button"
        className="step"
        aria-label={less}
        disabled={value <= min}
        onClick={() => onChange(value - 1)}
      >
        −
      </button>
      <button
        type="button"
        className="mini"
        aria-label={`${label}: ${value}`}
        onClick={() => onChange(value >= max ? min : value + 1)}
      >
        {children}
      </button>
      <button
        type="button"
        className="step"
        aria-label={more}
        disabled={value >= max}
        onClick={() => onChange(value + 1)}
      >
        +
      </button>
    </div>
  );
}

function GridIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <rect x="1" y="1" width="6" height="6" rx="1.2" />
      <rect x="9" y="1" width="6" height="6" rx="1.2" />
      <rect x="1" y="9" width="6" height="6" rx="1.2" />
      <rect x="9" y="9" width="6" height="6" rx="1.2" />
    </svg>
  );
}

function ListIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <rect x="1" y="2" width="14" height="2" rx="1" />
      <rect x="1" y="7" width="14" height="2" rx="1" />
      <rect x="1" y="12" width="14" height="2" rx="1" />
    </svg>
  );
}

/**
 * Grid or list, and how densely. Only the control for the active view is
 * visible; the other one fades and slides aside. "+" always means "more on
 * screen", which for rows means smaller ones.
 */
export function ViewControls({
  view,
  cols,
  rows,
  onView,
  onCols,
  onRows,
}: {
  view: LibraryView;
  cols: number;
  rows: number;
  onView: (view: LibraryView) => void;
  onCols: (value: number) => void;
  onRows: (value: number) => void;
}) {
  return (
    <>
      <div className="views" role="group" aria-label="View" data-view={view}>
        <span className="views__pill" aria-hidden="true" />
        <button type="button" aria-pressed={view === "grid"} onClick={() => onView("grid")}>
          <GridIcon />
          Grid
        </button>
        <button type="button" aria-pressed={view === "list"} onClick={() => onView("list")}>
          <ListIcon />
          List
        </button>
      </div>

      <div className="steppers" data-view={view}>
        <Stepper
          className="stepper--cols"
          label="Columns"
          value={cols}
          min={2}
          max={5}
          less="Fewer columns"
          more="More columns"
          inactive={view !== "grid"}
          onChange={onCols}
        >
          <MiniGrid cols={cols} />
        </Stepper>
        <Stepper
          className="stepper--rows"
          label="Row size"
          value={rows}
          min={1}
          max={3}
          less="Larger rows"
          more="Smaller rows"
          inactive={view !== "list"}
          onChange={onRows}
        >
          <MiniList rows={rows} />
        </Stepper>
      </div>
    </>
  );
}
