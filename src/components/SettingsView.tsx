import { open } from "@tauri-apps/plugin-dialog";
import { errorMessage, openOutputFolder, type AudioQuality, type DependencyStatus, type TranscriptFormat } from "../lib/api";
import type { Settings, Theme } from "../lib/settings";
import { Row, Segmented, Switch } from "./Controls";
import { Hero } from "./Hero";

type Props = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  deps: DependencyStatus[];
  onError: (message: string) => void;
};

export function SettingsView({ settings, update, deps, onError }: Props) {
  async function pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") update({ outDir: picked });
  }

  async function pickModel() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Whisper model", extensions: ["bin"] }],
    });
    if (typeof picked === "string") update({ whisperModelPath: picked });
  }

  const firstLang = settings.transcriptLangs[0] ?? "sr";

  return (
    <>
      <Hero
        kicker="Settings"
        title={
          <>
            Set once, <span className="accent">forget</span>
          </>
        }
        lead="Where things go and how they are made. The Convert screen keeps the choices you change every day."
      />

      <section className="glass">
        <h2 className="section-title">Files</h2>

        <Row title="Save to" hint={settings.outDir || "~/Documents/Tzz Library"}>
          <button
            type="button"
            className="ghost-button"
            onClick={() => openOutputFolder(settings.outDir || null).catch((c) => onError(errorMessage(c)))}
          >
            Show
          </button>
          <button type="button" className="ghost-button" onClick={pickFolder}>
            Change
          </button>
        </Row>

        <Row title="Audio quality" hint="VBR V0 is smaller and sounds the same; 320 is a fixed bitrate.">
          <Segmented<AudioQuality>
            label="Audio quality"
            value={settings.audioQuality}
            onChange={(audioQuality) => update({ audioQuality })}
            options={[
              { value: "v0", label: "VBR V0" },
              { value: "320", label: "320 kbps" },
            ]}
          />
        </Row>

        <Row title="Transcript format">
          <Segmented<TranscriptFormat>
            label="Transcript format"
            value={settings.transcriptFormat}
            onChange={(transcriptFormat) => update({ transcriptFormat })}
            options={[
              { value: "md", label: "Markdown" },
              { value: "pdf", label: "PDF" },
              { value: "both", label: "Both" },
            ]}
          />
        </Row>

        <Row title="Combine a queue into one document" hint="Each video keeps its own file as well.">
          <Switch
            label="Combine a queue into one document"
            checked={settings.combineQueue}
            onChange={(combineQueue) => update({ combineQueue })}
          />
        </Row>
      </section>

      <section className="glass">
        <h2 className="section-title">Transcription</h2>

        <Row
          title="Caption language to try first"
          hint="The original track always wins over a machine translation."
        >
          <Segmented<string>
            label="Caption language"
            value={firstLang}
            onChange={(lang) =>
              update({ transcriptLangs: lang === "sr" ? ["sr", "en"] : ["en", "sr"] })
            }
            options={[
              { value: "sr", label: "Serbian" },
              { value: "en", label: "English" },
            ]}
          />
        </Row>

        <Row
          title="Whisper model"
          hint={settings.whisperModelPath || "Not set. Videos without captions get no transcript."}
        >
          <button type="button" className="ghost-button" onClick={pickModel}>
            Choose
          </button>
        </Row>
      </section>

      <section className="glass">
        <h2 className="section-title">Appearance</h2>
        <Row title="Theme">
          <Segmented<Theme>
            label="Theme"
            value={settings.theme}
            onChange={(theme) => update({ theme })}
            options={[
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
            ]}
          />
        </Row>
      </section>

      <section className="glass">
        <h2 className="section-title">Tools</h2>
        {deps.map((dep) => (
          <Row
            key={dep.name}
            title={dep.name}
            hint={dep.found ? dep.path : `Not found. Install with brew install ${dep.name === "whisper-cli" ? "whisper-cpp" : dep.name}`}
          >
            <span className={`pill-status ${dep.found ? "pill-status--ok" : "pill-status--missing"}`}>
              {dep.found ? "Installed" : "Missing"}
            </span>
          </Row>
        ))}
      </section>
    </>
  );
}
