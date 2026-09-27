import { useLayoutEffect, useRef, type ReactNode } from "react";
import { ConvertIcon, LibraryIcon, MicIcon, SettingsIcon } from "./Icons";

export type View = "convert" | "record" | "library" | "settings";

const NAV: { id: View; label: string; icon: ReactNode }[] = [
  { id: "convert", label: "Convert", icon: <ConvertIcon size={16} /> },
  { id: "record", label: "Record", icon: <MicIcon size={16} /> },
  { id: "library", label: "Library", icon: <LibraryIcon size={16} /> },
  { id: "settings", label: "Settings", icon: <SettingsIcon size={16} /> },
];

type Status = {
  tone: "idle" | "busy" | "live" | "done";
  title: string;
  caption: string;
};

type Props = {
  view: View;
  onNavigate: (view: View) => void;
  status: Status;
  /** Controls that belong to the current screen, under the navigation. */
  children?: ReactNode;
};

function rows(onNavigate?: (view: View) => void) {
  return NAV.map((item) => (
    <button
      key={item.id}
      type="button"
      data-nav={item.id}
      tabIndex={onNavigate ? undefined : -1}
      onClick={onNavigate ? () => onNavigate(item.id) : undefined}
    >
      {item.icon}
      {item.label}
    </button>
  ));
}

/**
 * The window's only navigation.
 *
 * The selected row is a light pill that slides, and it carries a dark copy of
 * the whole list moving the opposite way -- so the text under it is dark
 * exactly where the pill covers it, mid-slide included. The copy is `inert`,
 * so Tab and a screen reader never reach the duplicate.
 */
export function Sidebar({ view, onNavigate, status, children }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const pill = useRef<HTMLSpanElement>(null);
  const copy = useRef<HTMLSpanElement>(null);
  const placed = useRef(false);

  useLayoutEffect(() => {
    const box = wrap.current;
    const light = pill.current;
    const dark = copy.current;
    if (!box || !light || !dark) return;

    const button = box.querySelector<HTMLElement>(`[data-nav="${view}"]`);
    if (!button) return;

    const y = button.getBoundingClientRect().top - box.getBoundingClientRect().top;
    // The first placement must not slide in from the top of the list.
    const instant = !placed.current;
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
    placed.current = true;
  }, [view]);

  return (
    <aside className="side" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <b>Tzz</b>
        <span>Lectures to notes</span>
      </div>

      <div className="nav" ref={wrap}>
        <span className="nav__pill" ref={pill} aria-hidden="true">
          <span className="nav__copy" ref={copy} inert>
            {rows()}
          </span>
        </span>
        {rows(onNavigate)}
      </div>

      {children}

      <div className={`status status--${status.tone}`}>
        <span className="status__dot" aria-hidden="true" />
        <span>
          <b>{status.title}</b>
          <small>{status.caption}</small>
        </span>
      </div>
    </aside>
  );
}
