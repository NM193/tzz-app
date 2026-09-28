import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  checkDependencies,
  defaultWhisperModel,
  setGlass,
  type DependencyStatus,
} from "./lib/api";
import { fileItem, isPlaylist, knownLink, pendingLink, type PendingLink } from "./lib/queue";
import { errorMessage, probePlaylist, probeVideo } from "./lib/api";
import { loadSettings, saveSettings, type Settings } from "./lib/settings";
import { STAGE_LABEL, useJobs } from "./lib/useJobs";
import { clock, useRecorder } from "./lib/useRecorder";
import { ConvertView } from "./components/ConvertView";
import { LibraryView } from "./components/LibraryView";
import { RecordView } from "./components/RecordView";
import { SettingsView } from "./components/SettingsView";
import { Sidebar, type View } from "./components/Sidebar";
import { LibraryControls } from "./components/LibraryControls";
import { PLAYLIST_LIMIT } from "./lib/queue";
import { ReaderControls } from "./components/ReaderControls";
import { ReaderView } from "./components/ReaderView";
import type { LibraryEntry, TranscriptDocument } from "./lib/api";
import { useLibrary } from "./lib/useLibrary";
import { scan } from "./lib/find";
import { readNotes, saveNote } from "./lib/api";

export default function App() {
  const [view, setView] = useState<View>("convert");
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const [deps, setDeps] = useState<DependencyStatus[]>([]);
  const [links, setLinks] = useState<PendingLink[]>([]);
  const [dragging, setDragging] = useState(false);
  const [libraryVersion, setLibraryVersion] = useState(0);

  const jobs = useJobs(settings);
  // The results list, shared: the hook measures it for the column re-flow.
  const resultsRef = useRef<HTMLOListElement>(null);
  const library = useLibrary(settings, libraryVersion, resultsRef);

  // The reader is a state of the Library, not a fifth screen: you get there by
  // opening something, and Back puts you where you were.
  const [reading, setReading] = useState<LibraryEntry | null>(null);
  const [doc, setDoc] = useState<TranscriptDocument | null>(null);
  const [showScreens, setShowScreens] = useState(true);
  /** A folder can hold two documents; this is the one being read. */
  const [which, setWhich] = useState<"transcript" | "notes">("transcript");
  const [goTo, setGoTo] = useState<number | null>(null);
  const [here, setHere] = useState(0);
  const [find, setFind] = useState("");
  /** The notes kept for the open transcript, by chapter title. */
  const [notes, setNotes] = useState<Map<string, string>>(new Map());
  const [hit, setHit] = useState(0);

  // One pass over the document numbers every match, so the reader can mark
  // them and the contents can say how many each chapter holds.
  const found = useMemo(() => scan(doc, find, showScreens), [doc, find, showScreens]);

  // Notes belong to the transcript, not to whichever document is being read.
  useEffect(() => {
    const transcript = reading?.markdownPath;
    if (!transcript) {
      setNotes(new Map());
      return;
    }
    let stale = false;
    readNotes(transcript)
      .then((kept) => !stale && setNotes(new Map(kept)))
      .catch(() => !stale && setNotes(new Map()));
    return () => {
      stale = true;
    };
  }, [reading?.markdownPath]);

  async function keepNote(chapter: string, body: string) {
    const entry = reading;
    if (!entry?.markdownPath) return;

    setNotes((kept) => {
      const next = new Map(kept);
      if (body.trim()) next.set(chapter, body.trim());
      else next.delete(chapter);
      return next;
    });

    try {
      const path = await saveNote(entry.markdownPath, entry.name, chapter, body);
      // The folder now holds a document it did not before, or no longer does.
      setReading({ ...entry, notesPath: path });
      setLibraryVersion((v) => v + 1);
    } catch (caught) {
      jobs.setError(errorMessage(caught));
    }
  }

  // A new query starts at the first match, never wherever the last one ended.
  useEffect(() => setHit(0), [find, doc]);

  function step(by: number) {
    if (found.total === 0) return;
    setHit((at) => (at + by + found.total) % found.total);
  }
  const recorder = useRecorder({
    // A finished recording joins the queue like any dropped file.
    onSaved: (path) => {
      setView("convert");
      void jobs.enqueue([fileItem(path)]);
    },
    onError: jobs.setError,
  });

  function update(patch: Partial<Settings>) {
    setSettings((prev) => ({ ...prev, ...patch }));
  }

  useEffect(() => {
    checkDependencies().then(setDeps).catch(() => setDeps([]));
  }, []);

  // Fill in a model found on disk, but never override a deliberate choice.
  useEffect(() => {
    if (settings.whisperModelPath) return;
    defaultWhisperModel()
      .then((path) => {
        if (!path) return;
        setSettings((prev) => (prev.whisperModelPath ? prev : { ...prev, whisperModelPath: path }));
      })
      .catch(() => {});
  }, []);

  useEffect(() => saveSettings(settings), [settings]);

  // The glass is a real window material, so the choice goes to macOS as well
  // as to the stylesheet. Failing to apply it only costs the effect.
  useEffect(() => {
    document.documentElement.dataset.surface = settings.surface;
    void setGlass(settings.surface === "glass").catch(() => {});
  }, [settings.surface]);

  useEffect(() => {
    document.documentElement.dataset.accent = settings.accent;
  }, [settings.accent]);

  useEffect(() => {
    document.documentElement.dataset.tint = settings.tint;
    document.documentElement.dataset.aurora = settings.aurora;
  }, [settings.tint, settings.aurora]);

  // Files dropped anywhere on the window start straight away.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragging(true);
      } else if (event.payload.type === "leave") {
        setDragging(false);
      } else if (event.payload.type === "drop") {
        setDragging(false);
        if (event.payload.paths.length > 0) {
          setView("convert");
          void jobs.enqueue(event.payload.paths.map((path) => fileItem(path)));
        }
      }
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  // The library changed; whoever is looking at it should see it.
  useEffect(() => {
    if (jobs.stage === "done") setLibraryVersion((v) => v + 1);
  }, [jobs.stage]);

  // The links on screen, for de-duplicating a paste without reading state
  // inside an updater -- StrictMode runs those twice.
  const linksRef = useRef<PendingLink[]>([]);
  linksRef.current = links;

  /**
   * A pasted link becomes a pill straight away and reads its own title in the
   * background. A playlist link becomes one pill that turns into all of them.
   */
  function addLinks(urls: string[]) {
    const known = new Set(linksRef.current.map((l) => l.url));
    const fresh = urls
      .filter((url) => !known.has(url))
      .map((url) => (isPlaylist(url) ? { ...pendingLink(url), playlist: true } : pendingLink(url)));

    if (fresh.length === 0) return;
    setLinks((prev) => [...prev, ...fresh]);

    for (const link of fresh) {
      if (link.playlist) void unpack(link);
      else void readTitle(link);
    }
  }

  async function readTitle(link: PendingLink) {
    try {
      const meta = await probeVideo(link.url);
      setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, title: meta.title } : l)));
    } catch {
      setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, failed: true } : l)));
    }
  }

  /** One playlist pill becomes a pill per video, titles already known. */
  async function unpack(link: PendingLink) {
    try {
      const playlist = await probePlaylist(link.url);
      if (playlist.items.length === 0) {
        setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, failed: true } : l)));
        jobs.setError("That playlist has nothing in it that can be fetched.");
        return;
      }

      setLinks((all) => {
        const known = new Set(all.filter((l) => l.id !== link.id).map((l) => l.url));
        const added = playlist.items
          .filter((item) => !known.has(item.url))
          .map((item) => knownLink(item.url, item.title));
        const at = all.findIndex((l) => l.id === link.id);
        if (at === -1) return all;
        return [...all.slice(0, at), ...added, ...all.slice(at + 1)];
      });

      if (playlist.items.length >= PLAYLIST_LIMIT) {
        jobs.setError(
          `That playlist is longer than ${PLAYLIST_LIMIT} videos. The first ${PLAYLIST_LIMIT} were added.`,
        );
      }
    } catch (caught) {
      setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, failed: true } : l)));
      jobs.setError(errorMessage(caught));
    }
  }

  const missing = useMemo(
    () => deps.filter((d) => !d.found && d.name !== "whisper-cli").map((d) => d.name),
    [deps],
  );
  const blocked =
    missing.length > 0
      ? `${missing.join(" and ")} ${missing.length === 1 ? "is" : "are"} not installed. Run: brew install ${missing.join(" ")}`
      : null;

  const status = recorder.recording
    ? { tone: "live" as const, title: `Recording ${clock(recorder.elapsed)}`, caption: recorder.routedTo ? `via ${recorder.routedTo}` : "Click Record to stop" }
    : jobs.running
      ? {
          tone: "busy" as const,
          title: `${STAGE_LABEL[jobs.stage]}${jobs.percent === null ? "" : ` ${Math.round(jobs.percent)}%`}`,
          caption: jobs.detail ?? "Working",
        }
      : jobs.stage === "done"
        ? { tone: "done" as const, title: "Finished", caption: "Files are in the library" }
        : { tone: "idle" as const, title: "Ready", caption: "Convert your lectures" };

  return (
    <div className={`app ${dragging ? "app--dragging" : ""}`}>
      <Sidebar view={view} onNavigate={setView} status={status}>
        {view === "library" &&
          (reading ? (
            <ReaderControls
              entry={reading}
              doc={doc}
              showScreens={showScreens}
              onShowScreens={setShowScreens}
              onBack={() => {
                setReading(null);
                setFind("");
              }}
              onGoTo={setGoTo}
              here={here}
              which={which}
              onWhich={(next) => {
                setWhich(next);
                setHere(0);
                setFind("");
              }}
              find={find}
              onFind={setFind}
              found={found}
              hit={hit}
              onStep={step}
            />
          ) : (
            <LibraryControls library={library} />
          ))}
      </Sidebar>

      <main className="stage">
        {view === "convert" && (
          <ConvertView
            settings={settings}
            update={update}
            jobs={jobs}
            links={links}
            onAddLinks={addLinks}
            onRemoveLink={(id) => setLinks((prev) => prev.filter((l) => l.id !== id))}
            onClearLinks={() => setLinks([])}
            dragging={dragging}
            blocked={blocked}
          />
        )}
        {view === "record" && <RecordView recorder={recorder} jobs={jobs} />}
        {view === "library" &&
          (reading ? (
            <ReaderView
              entry={reading}
              path={
                (which === "notes" ? reading.notesPath : reading.markdownPath) ??
                reading.markdownPath ??
                ""
              }
              showScreens={showScreens}
              onLoaded={setDoc}
              goTo={goTo}
              onArrived={() => setGoTo(null)}
              onHere={setHere}
              find={find}
              scan={found}
              hit={hit}
              notes={notes}
              onSaveNote={(chapter, body) => void keepNote(chapter, body)}
            />
          ) : (
            <LibraryView
              settings={settings}
              library={library}
              box={resultsRef}
              onOpen={(entry, open) => {
                setHere(0);
                setWhich(open);
                setReading(entry);
              }}
            />
          ))}
        {view === "settings" && (
          <SettingsView
            settings={settings}
            update={update}
            deps={deps}
            onError={jobs.setError}
          />
        )}
      </main>
    </div>
  );
}
