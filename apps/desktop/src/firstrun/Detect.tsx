import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../settings/api";
import { TermRadio } from "../settings/TermRadio";
import { errorText, type ProbeResult } from "../settings/types";

export const NONE_FOUND =
  "No local server found. Commands, apps and files work without one. You can connect a model later in Settings.";

const TITLES: Record<string, string> = {
  ollama: "Ollama",
  lmstudio: "LM Studio",
  llamacpp: "llama.cpp server",
};

/** Radio value for "no provider"; no server id can equal it. */
const SKIP = "";

/** Ask step body: offer running local servers as radios, nothing enabled until one is picked. */
export function Detect() {
  const [found, setFound] = useState<ProbeResult[] | null>(null);
  const [chosen, setChosen] = useState(SKIP);
  const committed = useRef(SKIP);
  const inflight = useRef(false);
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

  /** Select one server (or `SKIP`): enable the new choice, then turn the previous one off. */
  const choose = async (id: string) => {
    // One call at a time: a second arrow press mid-flight is dropped, not interleaved.
    if (inflight.current || id === committed.current) return;
    inflight.current = true;
    const prev = committed.current;
    try {
      if (id !== SKIP) await api.setProvider(id, { enabled: true });
      committed.current = id;
      setChosen(id);
      if (prev !== SKIP) await api.setProvider(prev, { enabled: false });
    } catch (e) {
      setErr(errorText(e));
      setChosen(committed.current);
    } finally {
      inflight.current = false;
    }
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
        <TermRadio
          legend="Ask provider"
          name="ask-provider"
          value={chosen}
          onChange={(id) => void choose(id)}
          options={[
            ...found.map((p) => ({
              value: p.id,
              label: TITLES[p.id] ?? p.id,
              hint: p.models.join(", ") || "No models listed",
            })),
            { value: SKIP, label: "skip, keep ask off", hint: "turn it on later in settings" },
          ]}
        />
      )}
      {err && (
        <p className="fr-status" data-kind="error" role="alert">
          {err}
        </p>
      )}
      <button type="button" className="btn" onClick={probe}>
        check again
      </button>
    </div>
  );
}
