import { useCallback, useEffect, useState } from "react";
import * as api from "../settings/api";
import { Toggle } from "../settings/Toggle";
import { errorText, type ProbeResult } from "../settings/types";

export const NONE_FOUND =
  "No local server found. Commands, apps, files and projects work without one. You can connect a model later in Settings.";

const TITLES: Record<string, string> = {
  ollama: "Ollama",
  lmstudio: "LM Studio",
  llamacpp: "llama.cpp server",
};

/** Step 2 body: offer running local servers, every switch starts off. */
export function Detect() {
  const [found, setFound] = useState<ProbeResult[] | null>(null);
  const [on, setOn] = useState<Record<string, boolean>>({});
  const [err, setErr] = useState<string | null>(null);

  const probe = useCallback(() => {
    setFound(null);
    setErr(null);
    api.probeProviders().then(
      (r) => setFound(r.filter((p) => p.reachable)),
      (e: unknown) => {
        setErr(errorText(e));
        setFound([]);
      },
    );
  }, []);
  useEffect(probe, [probe]);

  const toggle = (id: string, enabled: boolean) => {
    api.setProvider(id, { enabled }).then(
      () => setOn((o) => ({ ...o, [id]: enabled })),
      (e: unknown) => setErr(errorText(e)),
    );
  };

  return (
    <div className="fr-detect">
      {found === null ? (
        <p className="s-help" role="status">
          Looking for local servers…
        </p>
      ) : found.length === 0 ? (
        <p className="s-help">{NONE_FOUND}</p>
      ) : (
        <ul className="fr-servers">
          {found.map((p) => (
            <li key={p.id} className="fr-server">
              <div>
                <div>{TITLES[p.id] ?? p.id}</div>
                <div className="s-help">{p.models.join(", ") || "No models listed"}</div>
              </div>
              <Toggle
                checked={on[p.id] ?? false}
                onChange={(v) => toggle(p.id, v)}
                label={`Use ${TITLES[p.id] ?? p.id}`}
              />
            </li>
          ))}
        </ul>
      )}
      {err && (
        <p className="fr-status" data-kind="error" role="alert">
          {err}
        </p>
      )}
      <button type="button" className="fr-btn" onClick={probe}>
        Check again
      </button>
    </div>
  );
}
