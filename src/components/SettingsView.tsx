import { open } from "@tauri-apps/plugin-dialog";
import {
  errorMessage,
  openOutputFolder,
  type AudioQuality,
  type DependencyStatus,
} from "../lib/api";
import type { Accent, Aurora, Settings, Surface, Tint } from "../lib/settings";
import { Segmented, Setting, Switch } from "./Controls";

type Props = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  deps: DependencyStatus[];
  onError: (message: string) => void;
};

const BREW: Record<string, string> = {
  "yt-dlp": "yt-dlp",
  ffmpeg: "ffmpeg",
  "whisper-cli": "whisper-cpp",
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

  return (
    <>
      <h1 className="title">Settings</h1>
      <p className="lede">
        Set once, forget. What changes every day lives on the Convert screen instead.
      </p>

      <div className="section">
        <h2>Appearance</h2>

        <Setting
          title="Window"
          hint="Glass shows your desktop through the window. Aurora paints its own sky, so it looks the same whatever your wallpaper is. Solid is plain and the easiest to read."
        >
          <Segmented<Surface>
            label="Window"
            value={settings.surface}
            onChange={(surface) => update({ surface })}
            options={[
              { value: "solid", label: "Solid" },
              { value: "glass", label: "Glass" },
              { value: "aurora", label: "Aurora" },
            ]}
          />
        </Setting>

        {settings.surface === "aurora" && (
          <Setting title="Sky" hint="Which colours the app paints behind everything.">
            <Segmented<Aurora>
              label="Sky"
              value={settings.aurora}
              onChange={(aurora) => update({ aurora })}
              options={[
                { value: "dusk", label: "Dusk" },
                { value: "ember", label: "Ember" },
                { value: "moss", label: "Moss" },
              ]}
            />
          </Setting>
        )}

        {settings.surface !== "solid" && (
          <Setting
            title="Tint"
            hint="How much of what is behind comes through. Clear shows the most."
          >
            <Segmented<Tint>
              label="Tint"
              value={settings.tint}
              onChange={(tint) => update({ tint })}
              options={[
                { value: "warm", label: "Warm" },
                { value: "cool", label: "Cool" },
                { value: "ink", label: "Ink" },
                { value: "clear", label: "Clear" },
              ]}
            />
          </Setting>
        )}

        <Setting title="Accent" hint="The one colour the app uses for anything live.">
          <Segmented<Accent>
            label="Accent"
            value={settings.accent}
            onChange={(accent) => update({ accent })}
            options={[
              { value: "amber", label: "Amber" },
              { value: "green", label: "Green" },
            ]}
          />
        </Setting>
      </div>

      <div className="section">
        <h2>Files</h2>

        <Setting title="Save to" hint={settings.outDir || "~/Documents/Tzz Library"}>
          <button
            type="button"
            className="ghost ghost--small"
            onClick={() =>
              openOutputFolder(settings.outDir || null).catch((c) => onError(errorMessage(c)))
            }
          >
            Show
          </button>
          <button type="button" className="ghost ghost--small" onClick={pickFolder}>
            Change
          </button>
        </Setting>

        <Setting
          title="Audio quality"
          hint="VBR V0 is smaller and sounds the same; 320 is a fixed bitrate."
        >
          <Segmented<AudioQuality>
            label="Audio quality"
            value={settings.audioQuality}
            onChange={(audioQuality) => update({ audioQuality })}
            options={[
              { value: "v0", label: "VBR V0" },
              { value: "320", label: "320 kbps" },
            ]}
          />
        </Setting>

        <Setting
          title="Combine a queue into one document"
          hint="Each lecture keeps its own file as well."
        >
          <Switch
            label="Combine a queue into one document"
            checked={settings.combineQueue}
            onChange={(combineQueue) => update({ combineQueue })}
          />
        </Setting>
      </div>

      <div className="section">
        <h2>Transcription</h2>

        <Setting
          title="Caption language to try first"
          hint="The original track always wins over a machine translation."
        >
          <Segmented<string>
            label="Caption language"
            value={settings.transcriptLangs[0] ?? "sr"}
            onChange={(lang) =>
              update({ transcriptLangs: lang === "sr" ? ["sr", "en"] : ["en", "sr"] })
            }
            options={[
              { value: "sr", label: "Serbian" },
              { value: "en", label: "English" },
            ]}
          />
        </Setting>

        <Setting
          title="Whisper model"
          hint={
            settings.whisperModelPath ||
            "Not set. Lectures without captions get no transcript."
          }
        >
          <button type="button" className="ghost ghost--small" onClick={pickModel}>
            Choose
          </button>
        </Setting>
      </div>

      <div className="section">
        <h2>Tools</h2>
        {deps.map((dep) => (
          <Setting
            key={dep.name}
            title={dep.name}
            hint={dep.found ? dep.path : `Install with: brew install ${BREW[dep.name] ?? dep.name}`}
          >
            <span className={`state ${dep.found ? "state--ok" : ""}`}>
              {dep.found ? "Ready" : "Missing"}
            </span>
          </Setting>
        ))}
      </div>
    </>
  );
}
