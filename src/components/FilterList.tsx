import { useEffect, useLayoutEffect, useRef, type PointerEvent } from "react";
import { FILTERS, type Filter } from "../lib/library";

type Props = {
  active: Filter;
  counts: Record<Filter, number>;
  onSelect: (filter: Filter) => void;
};

function rows(
  active: Filter,
  counts: Record<Filter, number>,
  onSelect?: (filter: Filter) => void,
) {
  return FILTERS.map((filter) => (
    <li key={filter.id}>
      <button
        type="button"
        data-filter={filter.id}
        data-zero={counts[filter.id] ? undefined : ""}
        aria-pressed={onSelect ? active === filter.id : undefined}
        tabIndex={onSelect ? undefined : -1}
        onClick={onSelect ? () => onSelect(filter.id) : undefined}
      >
        <span>{filter.label}</span>
        <i>{counts[filter.id]}</i>
      </button>
    </li>
  ));
}

/**
 * The selected row is a light pill carrying a dark copy of the list that moves
 * the opposite way -- so the text under it is dark exactly where it is covered.
 * Hover is a separate outline that slides to whichever row the pointer is over.
 */
export function FilterList({ active, counts, onSelect }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const pill = useRef<HTMLSpanElement>(null);
  const copy = useRef<HTMLSpanElement>(null);
  const hover = useRef<HTMLSpanElement>(null);
  const placed = useRef(false);

  function place(instant: boolean) {
    const box = wrap.current;
    const light = pill.current;
    const dark = copy.current;
    if (!box || !light || !dark) return;
    const button = box.querySelector<HTMLElement>(`.cats__list [data-filter="${active}"]`);
    if (!button) return;

    const y = button.getBoundingClientRect().top - box.getBoundingClientRect().top;
    if (instant) {
      light.style.transition = "none";
      dark.style.transition = "none";
    }
    light.style.transform = `translateY(${y}px)`;
    light.style.height = `${button.offsetHeight}px`;
    dark.style.transform = `translateY(${-y}px)`;
    if (instant) {
      void light.offsetHeight;
      light.style.transition = "";
      dark.style.transition = "";
    }
  }

  useLayoutEffect(() => {
    place(!placed.current);
    placed.current = true;
  }, [active]);

  // A width change moves the rows; the pill is put back without sliding. The
  // observer's first call arrives immediately and must not cut off a slide.
  useEffect(() => {
    const box = wrap.current;
    if (!box) return;
    let first = true;
    const observer = new ResizeObserver(() => {
      if (first) {
        first = false;
        return;
      }
      place(true);
    });
    observer.observe(box);
    return () => observer.disconnect();
  });

  function showHover(event: PointerEvent<HTMLDivElement>) {
    const box = wrap.current;
    const outline = hover.current;
    const button = (event.target as Element).closest<HTMLElement>(".cats__list button");
    if (!box || !outline || !button) return;

    const hidden = outline.style.opacity !== "1";
    const y = button.getBoundingClientRect().top - box.getBoundingClientRect().top;
    if (hidden) outline.style.transition = "none";
    outline.style.transform = `translateY(${y}px)`;
    outline.style.height = `${button.offsetHeight}px`;
    if (hidden) {
      void outline.offsetHeight;
      outline.style.transition = "";
    }
    outline.style.opacity = "1";
  }

  return (
    <div
      className="cats"
      ref={wrap}
      onPointerOver={showHover}
      onPointerLeave={() => {
        if (hover.current) hover.current.style.opacity = "0";
      }}
    >
      <span className="cats__hover" ref={hover} aria-hidden="true" />
      <ul className="cats__list">{rows(active, counts, onSelect)}</ul>
      <span className="cats__pill" ref={pill} aria-hidden="true">
        <span className="cats__copy" ref={copy} inert>
          <ul className="cats__list">{rows(active, counts)}</ul>
        </span>
      </span>
    </div>
  );
}
