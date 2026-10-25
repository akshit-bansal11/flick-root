import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import { keyHelp } from "./MaskedKey";
import { OnOff } from "./OnOff";
import { SettingRow } from "./Row";
import { type EnginePatch, type EngineProbe, type EngineView, errorText } from "./types";
import "./features.css";
import "./providers.css";

const STATUS: Record<EngineProbe["status"], string> = {
  reachable: "reachable",
  not_running: "not running",
  no_key: "no key",
  no_provider: "no provider",
};

/** What each engine does, shown in the preview column. */
const ABOUT: Record<string, string> = {
  llm: "Uses your Query provider (AI providers page). Receives file names only.",
  laya: "Runs locally: start laya-serve from the Laya package (see the docs page for setup), then point this at it. Default http://localhost:8000.",
  custom:
    "Any server that implements /v1/systemone. Set its URL, then optionally add a key (sent only to that host).",
};

/** Secret name for an engine key: engine.<id>@<host> (Rust re-validates). */
const secretName = (e: EngineView) => `engine.${e.id}@${e.key_host ?? ""}`;

function statusText(e: EngineView, probe?: EngineProbe): string {
  if (e.needs_key && !e.key_present) return STATUS.no_key;
  if (!probe) return e.enabled ? "checking…" : "off";
  return STATUS[probe.status];
}

interface RowProps {
  e: EngineView;
  probe?: EngineProbe;
  apply: (id: string, patch: EnginePatch) => Promise<void>;
  reload: () => void;
}

function Row({ e, probe, apply, reload }: RowProps) {
  const [url, setUrl] = useState(e.base_url);
  useEffect(() => setUrl(e.base_url), [e.base_url]);
  const [dialog, setDialog] = useState(false);
  const hasUrl = e.id !== "llm";
  const canKey = hasUrl && !!e.key_host;

  const status = statusText(e, probe);
  const ok = probe?.status === "reachable";

  return (
    <>
      <SettingRow
        label={e.title}
        value={ok ? status : undefined}
        hint={ok ? undefined : status}
        help={ABOUT[e.id]}
        preview={{
          title: `# ${e.title.toLowerCase()}`,
          body: `${ABOUT[e.id] ?? "Asked about ambiguous requests when enabled."} Status: ${status}.`,
        }}
      >
        <span className="s-add">
          {canKey && !e.key_present && (
            <button
              type="button"
              className="btn"
              aria-label={`Add key for ${e.title}`}
              onClick={() => setDialog(true)}
            >
              add key
            </button>
          )}
          <OnOff
            checked={e.enabled}
            label={`Enable ${e.title}`}
            onChange={(v) => void apply(e.id, { enabled: v })}
          />
        </span>
      </SettingRow>
      {hasUrl && (
        <div className="f-form">
          <label className="f-field">
            {`${e.title} base URL`}
            <input className="s-input" value={url} onChange={(ev) => setUrl(ev.target.value)} />
          </label>
          {url !== e.base_url && (
            <div className="s-add">
              <button
                type="button"
                className="btn"
                aria-label={`Save ${e.title} URL`}
                onClick={() => apply(e.id, { baseUrl: url })}
              >
                save url
              </button>
            </div>
          )}
        </div>
      )}
      {e.key_present && (
        <SettingRow
          label="key"
          value={`Key stored for ${e.key_host}`}
          help={keyHelp(e.key_host ?? "")}
          preview={{
            title: "# key",
            body: keyHelp(e.key_host ?? ""),
          }}
        >
          <span className="s-add">
            <button
              type="button"
              className="btn"
              aria-label={`Replace key for ${e.title}`}
              onClick={() => setDialog(true)}
            >
              replace
            </button>
            <button
              type="button"
              className="btn"
              aria-label={`Delete key for ${e.title}`}
              onClick={() => api.deleteSecret(secretName(e)).then(reload, () => undefined)}
            >
              delete
            </button>
          </span>
        </SettingRow>
      )}
      {dialog && (
        <AddKeyDialog
          providerName={e.title}
          host={e.key_host ?? ""}
          keyless={false}
          mode={e.key_present ? "replace" : "add"}
          onCancel={() => setDialog(false)}
          onSave={async (value, acknowledged) => {
            await api.setSecret(secretName(e), value, acknowledged);
            setDialog(false);
            reload();
          }}
        />
      )}
    </>
  );
}

/** Decision engine page: the offline parser plus opt-in engines for ambiguous requests. */
export function Decision() {
  const [list, setList] = useState<EngineView[]>([]);
  const [probes, setProbes] = useState<Record<string, EngineProbe>>({});
  const [err, setErr] = useState<string | null>(null);
  const [cand, setCand] = useState("");
  const [thr, setThr] = useState("");

  const sync = useCallback((l: EngineView[]) => {
    setList(l);
    setCand(String(l[0]?.candidates ?? ""));
    setThr(String(l[0]?.threshold ?? ""));
  }, []);
  const reload = useCallback(() => {
    api.listEngines().then(sync, (e) => setErr(errorText(e)));
    api.probeDecisionEngines().then(
      (r) => setProbes(Object.fromEntries(r.map((x) => [x.id, x]))),
      (e) => setErr(errorText(e)),
    );
  }, [sync]);
  useEffect(reload, [reload]);

  const apply = async (id: string, patch: EnginePatch) => {
    try {
      sync(await api.setEngine(id, patch));
      setErr(null);
      api.probeDecisionEngines().then(
        (r) => setProbes(Object.fromEntries(r.map((x) => [x.id, x]))),
        () => undefined,
      );
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <section aria-label="Decision engine">
      <h1 className="s-title">Decision engine</h1>
      <p className="s-desc s-prose">
        The built-in parser always runs, offline. Engines are only asked about ambiguous requests,
        and receive file names and parent folders only, never file contents or keys.
      </p>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
      <h2 className="sec"># engines</h2>
      <SettingRow
        label="Built-in parser"
        value="always on"
        preview={{
          title: "# built-in parser",
          body: "Always runs, offline. Engines are only asked when it cannot decide.",
        }}
      >
        <OnOff checked label="Enable Built-in parser" disabled onChange={() => undefined} />
      </SettingRow>
      {[...list]
        .sort((a, b) => a.order - b.order)
        .map((e) => (
          <Row key={e.id} e={e} probe={probes[e.id]} apply={apply} reload={reload} />
        ))}
      <h2 className="sec"># limits</h2>
      <div className="f-form">
        <label className="f-field">
          Candidates (1 to 8)
          <input
            className="s-input"
            value={cand}
            inputMode="numeric"
            onChange={(ev) => setCand(ev.target.value)}
          />
        </label>
        <label className="f-field">
          Threshold (0 to 1)
          <input
            className="s-input"
            value={thr}
            inputMode="decimal"
            onChange={(ev) => setThr(ev.target.value)}
          />
        </label>
        <div className="s-add">
          <button
            type="button"
            className="btn pri"
            onClick={() => apply("laya", { candidates: Number(cand), threshold: Number(thr) })}
          >
            save limits
          </button>
        </div>
      </div>
    </section>
  );
}
