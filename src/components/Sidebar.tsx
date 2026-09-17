import type { ReactNode } from "react";
import { ConvertIcon, LibraryIcon, Logo, MicIcon, SettingsIcon } from "./Icons";

export type View = "convert" | "record" | "library" | "settings";

const NAV: { id: View; label: string; icon: ReactNode }[] = [
  { id: "convert", label: "Convert", icon: <ConvertIcon /> },
  { id: "record", label: "Record", icon: <MicIcon /> },
  { id: "library", label: "Library", icon: <LibraryIcon /> },
  { id: "settings", label: "Settings", icon: <SettingsIcon /> },
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
};

export function Sidebar({ view, onNavigate, status }: Props) {
  return (
    <aside className="sidebar" data-tauri-drag-region>
      {/* Room for the traffic lights, which sit on top of the window. */}
      <div className="sidebar__brand" data-tauri-drag-region>
        <span className="sidebar__mark">
          <Logo />
        </span>
        <span>
          <strong>Tzz App</strong>
          <small>Lectures to notes</small>
        </span>
      </div>

      <nav className="sidebar__nav">
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nav ${view === item.id ? "nav--active" : ""}`}
            aria-current={view === item.id ? "page" : undefined}
            onClick={() => onNavigate(item.id)}
          >
            {item.icon}
            {item.label}
          </button>
        ))}
      </nav>

      <div className={`sidebar__status sidebar__status--${status.tone}`}>
        <span className="sidebar__dot" aria-hidden="true" />
        <span>
          <strong>{status.title}</strong>
          <small>{status.caption}</small>
        </span>
      </div>
    </aside>
  );
}
