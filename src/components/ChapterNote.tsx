import { useEffect, useRef, useState } from "react";

type Props = {
  note: string;
  onSave: (body: string) => void;
};

/**
 * What you brought back for this chapter.
 *
 * The app writes nothing here. You copy the chapter out, ask whatever you
 * like, and paste the answer in -- so the note is yours, and so is the choice
 * of what produced it.
 */
export function ChapterNote({ note, onSave }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(note);
  const field = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setDraft(note);
    setEditing(false);
  }, [note]);

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

/** The button that starts a note, shown when there is none yet. */
export function AddNote({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="icon-button" onClick={onClick}>
      Add a note
    </button>
  );
}
