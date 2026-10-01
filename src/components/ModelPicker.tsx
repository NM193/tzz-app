import { useEffect, useState } from "react";
import {
  downloadWhisperModel,
  errorMessage,
  onModelProgress,
  whisperModels,
  type WhisperModel,
} from "../lib/api";

type Props = {
  /** Called with the path once a model is on disk. */
  onInstalled: (path: string) => void;
};

const gigabytes = (bytes: number) =>
  bytes >= 1_000_000_000
    ? `${(bytes / 1_000_000_000).toFixed(1)} GB`
    : `${Math.round(bytes / 1_000_000)} MB`;

/**
 * Choosing and fetching a Whisper model.
 *
 * None of them can travel inside the app -- the one worth having is a gigabyte
 * and a half -- so a fresh Mac has to be offered the choice rather than handed
 * a setting that points at nothing.
 */
export function ModelPicker({ onInstalled }: Props) {
  const [models, setModels] = useState<WhisperModel[]>([]);
  const [getting, setGetting] = useState<string | null>(null);
  const [percent, setPercent] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => whisperModels().then(setModels).catch(() => setModels([]));

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    const unlisten = onModelProgress((id, at) => {
      if (id === getting) setPercent(at);
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, [getting]);

  async function fetchIt(id: string) {
    setGetting(id);
    setPercent(0);
    setError(null);
    try {
      const path = await downloadWhisperModel(id);
      await refresh();
      onInstalled(path);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setGetting(null);
    }
  }

  return (
    <div className="models">
      {models.map((model) => (
        <div key={model.id} className="model">
          <div className="model__text">
            <b>
              {model.name}
              <em>{gigabytes(model.bytes)}</em>
            </b>
            <span>{model.note}</span>
          </div>

          {model.installed ? (
            <span className="state state--ok">On this Mac</span>
          ) : getting === model.id ? (
            <span className="model__progress">
              <span className="track">
                <i style={{ width: `${percent}%` }} />
              </span>
              {Math.round(percent)}%
            </span>
          ) : (
            <button
              type="button"
              className="ghost ghost--small"
              disabled={getting !== null}
              onClick={() => void fetchIt(model.id)}
            >
              Get it
            </button>
          )}
        </div>
      ))}

      {error && <p className="notice notice--loud">{error}</p>}
    </div>
  );
}
