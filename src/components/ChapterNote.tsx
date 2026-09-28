import { useEffect, useRef, useState } from "react";

type Props = {
  note: string;
  /** Open straight into the field. A note with no text to show would be blank. */
  start?: boolean;
  onSave: (body: string) => void;
  onCancel: () => void;
};

/**
 * What you brought back for this chapter.
 *
 * The app writes nothing here. You copy the chapter out, ask whatever you
 * like, and paste the answer in -- so the note is yours, and so is the choice
 * of what produced it.
 */
export function ChapterNote({ note, start = false, onSave, onCancel }: Props) {
  const [editing, setEditing] = useState(start);
  const [draft, setDraft] = useState(note);
  const field = useRef<HTMLTextAreaElement>(null);

  // No effect resets this from `note`: the reader gives the component a key
  // that carries the note, so a changed note arrives as a fresh component.

  useEffect(() => {
    if (!editing) return;
    const area = field.current;
    if (!area) return;
    area.focus();
    area.setSelectionRange(area.value.length, area.value.length);
  }, [editing]);

  if (editing) {
    return (
      <div className="note note--editing">
        <textarea
          ref={field}
          className="note__field"
          value={draft}
          placeholder="Paste what you got back"
          spellCheck={false}
          rows={Math.min(20, Math.max(5, draft.split("\n").length + 1))}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            // Enter makes a paragraph; saving is deliberate.
            if (event.key === "Escape") {
              setDraft(note);
              setEditing(false);
              onCancel();
            }
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              onSave(draft);
              setEditing(false);
            }
          }}
        />
        <div className="note__actions">
          <button
            type="button"
            className="ghost ghost--small"
            onClick={() => {
              setDraft(note);
              setEditing(false);
              onCancel();
            }}
          >
            Cancel
          </button>
          <button
            type="button"
            className="cta cta--small"
            onClick={() => {
              onSave(draft);
              setEditing(false);
            }}
          >
            Save
          </button>
        </div>
      </div>
    );
  }

  if (!note) return null;

  return (
    <div className="note">
      {note.split(/\n{2,}/).map((paragraph, index) => (
        <p key={index}>{paragraph}</p>
      ))}
      <button type="button" className="note__edit" onClick={() => setEditing(true)}>
        Edit
      </button>
    </div>
  );
}
