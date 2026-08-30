import { useRef, useState } from "react";
import { parseUrlList, shortenUrl, type PendingLink } from "../lib/queue";

type Props = {
  links: PendingLink[];
  onAdd: (urls: string[]) => void;
  onRemove: (id: string) => void;
  disabled: boolean;
};

/**
 * A field that turns pasted links into pills.
 *
 * Paste is the main path -- a course is copied a link at a time from the
 * browser -- so pasting several lines at once adds several pills.
 */
export function LinkField({ links, onAdd, onRemove, disabled }: Props) {
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  function commit(text: string) {
    const urls = parseUrlList(text);
    if (urls.length > 0) {
      onAdd(urls);
      setDraft("");
    }
  }

  return (
    <div
      className={`link-field ${disabled ? "link-field--disabled" : ""}`}
      onClick={() => inputRef.current?.focus()}
    >
      {links.map((link) => (
        <span
          key={link.id}
          className={`pill ${link.title === null && !link.failed ? "pill--loading" : ""} ${
            link.failed ? "pill--failed" : ""
          }`}
          title={link.url}
        >
          {link.title ?? (link.failed ? shortenUrl(link.url) : "reading title...")}
          <button
            type="button"
            className="pill__remove"
            aria-label="Remove link"
            disabled={disabled}
            onClick={(event) => {
              event.stopPropagation();
              onRemove(link.id);
            }}
          >
            ×
          </button>
        </span>
      ))}

      <input
        ref={inputRef}
        className="link-field__input"
        type="text"
        value={draft}
        placeholder={links.length === 0 ? "Paste YouTube links" : ""}
        spellCheck={false}
        autoComplete="off"
        disabled={disabled}
        onChange={(event) => setDraft(event.target.value)}
        onPaste={(event) => {
          event.preventDefault();
          commit(event.clipboardData.getData("text"));
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commit(draft);
          }
          if (event.key === "Backspace" && draft === "" && links.length > 0) {
            onRemove(links[links.length - 1].id);
          }
        }}
      />
    </div>
  );
}
