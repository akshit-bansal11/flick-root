import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import { type EnginePatch, type EngineProbe, type EngineView, errorText } from "./types";
import "./features.css";
import "./providers.css";

const STATUS: Record<EngineProbe["status"], string> = {
  reachable: "reachable",
  not_running: "not running",
  no_key: "no key",
  no_provider: "no provider",
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

  return (
    <li className="pv-row">
      <div className="pv-head">
        <input
          type="checkbox"
          aria-label={`Enable ${e.title}`}
          checked={e.enabled}
          onChange={(ev) => apply(e.id, { enabled: ev.target.checked })}
        />
        <span className="f-tile" aria-hidden="true">
          {e.title.charAt(0)}
        </span>
        <span className="pv-title">{e.title}</span>
        <span className="pv-stat" data-ok={probe?.status === "reachable"}>
          {statusText(e, probe)}
        </span>
        {canKey && !e.key_present && (
          <button type="button" onClick={() => setDialog(true)}>
            Add key
          </button>
        )}
      </div>
      {e.id === "llm" && (
        <p className="s-help">
          Uses your Query provider (AI providers page). Receives file names only.
        </p>
      )}
      {e.id === "laya" && (
        <p className="s-help">
          Runs locally: start <code>laya-serve</code> from the Laya package (see the docs page for
          setup), then point this at it. Default http://localhost:8000.
        </p>
      )}
      {hasUrl && (
        <div className="pv-opts">
          <label>
            {`${e.title} base URL`}
            <input value={url} onChange={(ev) => setUrl(ev.target.value)} />
          </label>
          {url !== e.base_url && (
            <button type="button" onClick={() => apply(e.id, { baseUrl: url })}>
              {`Save ${e.title} URL`}
            </button>
          )}
        </div>
      )}
      {e.key_present && (
        <div className="pv-head">
          <span className="s-help">{`Key stored for ${e.key_host}`}</span>
          <button type="button" onClick={() => setDialog(true)}>
            Replace key
          </button>
          <button
            type="button"
            onClick={() => api.deleteSecret(secretName(e)).then(reload, () => undefined)}
          >
            Delete key
          </button>
        </div>
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
    </li>
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
    <section className="pv-shell" aria-label="Decision engine">
      <h1 className="s-title">Decision engine</h1>
      <p className="s-desc">
        The built-in parser always runs, offline. Engines are only asked about ambiguous requests,
        and receive file names and parent folders only, never file contents or keys.
      </p>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
      <ul>
        <li className="pv-row">
          <div className="pv-head">
            <input type="checkbox" aria-label="Enable Built-in parser" checked disabled />
            <span className="f-tile" aria-hidden="true">
              B
            </span>
            <span className="pv-title">Built-in parser</span>
            <span className="pv-stat" data-ok="true">
              always on
            </span>
          </div>
        </li>
        {[...list]
          .sort((a, b) => a.order - b.order)
          .map((e) => (
            <Row key={e.id} e={e} probe={probes[e.id]} apply={apply} reload={reload} />
          ))}
      </ul>
      <h2 className="pv-group">Limits</h2>
      <div className="pv-opts">
        <label>
          Candidates (1 to 8)
          <input value={cand} inputMode="numeric" onChange={(ev) => setCand(ev.target.value)} />
        </label>
        <label>
          Threshold (0 to 1)
          <input value={thr} inputMode="decimal" onChange={(ev) => setThr(ev.target.value)} />
        </label>
        <button
          type="button"
          className="s-btn-primary"
          onClick={() => apply("laya", { candidates: Number(cand), threshold: Number(thr) })}
        >
          Save limits
        </button>
      </div>
    </section>
  );
}
