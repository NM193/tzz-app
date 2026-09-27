import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { checkDependencies, defaultWhisperModel, setGlass, type DependencyStatus } from "./lib/api";
import { fileItem, pendingLink, type PendingLink } from "./lib/queue";
import { probeVideo } from "./lib/api";
import { loadSettings, saveSettings, type Settings } from "./lib/settings";
import { STAGE_LABEL, useJobs } from "./lib/useJobs";
import { clock, useRecorder } from "./lib/useRecorder";
import { ConvertView } from "./components/ConvertView";
import { LibraryView } from "./components/LibraryView";
import { RecordView } from "./components/RecordView";
import { SettingsView } from "./components/SettingsView";
import { Sidebar, type View } from "./components/Sidebar";
import { LibraryControls } from "./components/LibraryControls";
import { useLibrary } from "./lib/useLibrary";

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

  /// Titles are read in the background; a link stays usable either way.
  function addLinks(urls: string[]) {
    setLinks((prev) => {
      const known = new Set(prev.map((l) => l.url));
      const fresh = urls.filter((url) => !known.has(url)).map(pendingLink);

      for (const link of fresh) {
        probeVideo(link.url)
          .then((meta) =>
            setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, title: meta.title } : l))),
          )
          .catch(() =>
            setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, failed: true } : l))),
          );
      }

      return [...prev, ...fresh];
    });
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
        {view === "library" && <LibraryControls library={library} />}
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
        {view === "library" && (
          <LibraryView settings={settings} library={library} box={resultsRef} />
        )}
        {view === "settings" && (
          <SettingsView settings={settings} update={update} deps={deps} onError={jobs.setError} />
        )}
      </main>
    </div>
  );
}
