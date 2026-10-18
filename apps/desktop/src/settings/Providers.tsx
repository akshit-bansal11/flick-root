import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import { MaskedKey } from "./MaskedKey";
import { errorText, type ProbeResult, type ProviderGroup, type ProviderView } from "./types";
import "./providers.css";

const GROUPS: [ProviderGroup, string][] = [
  ["local", "Local"],
  ["free", "Free gateways"],
  ["own_key", "Your own key"],
];

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

/** Secret name for a provider key: provider.<id>@<host>, host from the base URL (Rust re-validates). */
const secretName = (p: ProviderView) => `provider.${p.id}@${hostOf(p.base_url)}`;

function statusText(p: ProviderView, probe?: ProbeResult): string {
  if (p.key_required) return p.key ? "key set" : "no key";
  if (!probe) return "not probed";
  return probe.reachable
    ? `detected${probe.models[0] ? ` · ${probe.models[0]}` : ""}`
    : "not running";
}

interface RowProps {
  p: ProviderView;
  probe?: ProbeResult;
  index: number;
  count: number;
  onMove: (dir: -1 | 1) => void;
  reload: () => void;
}

function Row({ p, probe, index, count, onMove, reload }: RowProps) {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState(p.base_url);
  const [model, setModel] = useState(p.model);
  const [dialog, setDialog] = useState<"add" | "replace" | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const hostChanged = hostOf(url) !== hostOf(p.base_url);

  const run = async (f: () => Promise<unknown>) => {
    try {
      await f();
      setErr(null);
      reload();
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <li className="pv-row">
      <div className="pv-head">
        <input
          type="checkbox"
          aria-label={`Enable ${p.title}`}
          checked={p.enabled}
          onChange={(e) => run(() => api.setProvider(p.id, { enabled: e.target.checked }))}
        />
        <span>{p.title}</span>
        <span className="s-help">{statusText(p, probe)}</span>
        {p.key_required && !p.key && (
          <button type="button" onClick={() => setDialog("add")}>
            Add key
          </button>
        )}
        <button type="button" aria-expanded={open} onClick={() => setOpen(!open)}>
          Options
        </button>
      </div>
      {p.key && (
        <MaskedKey info={p.key} onReplace={() => setDialog("replace")} onDeleted={reload} />
      )}
      {open && (
        <div className="pv-opts">
          <label>
            Base URL
            <input value={url} onChange={(e) => setUrl(e.target.value)} />
          </label>
          {hostChanged && p.key && (
            <p className="pv-warn" role="alert">
              Changing the host deletes the stored key for this provider when you save.
            </p>
          )}
          <label>
            Model
            <input value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
          <div className="pv-head">
            <button
              type="button"
              onClick={() => run(() => api.setProvider(p.id, { baseUrl: url, model }))}
            >
              Save options
            </button>
            <button
              type="button"
              aria-label={`Move ${p.title} up`}
              disabled={index === 0}
              onClick={() => onMove(-1)}
            >
              ↑
            </button>
            <button
              type="button"
              aria-label={`Move ${p.title} down`}
              disabled={index === count - 1}
              onClick={() => onMove(1)}
            >
              ↓
            </button>
            <span className="s-help">Fallback order</span>
          </div>
        </div>
      )}
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
      {dialog && (
        <AddKeyDialog
          providerName={p.title}
          host={hostOf(p.base_url)}
          keyless={!p.key_required}
          mode={dialog}
          onCancel={() => setDialog(null)}
          onSave={async (value) => {
            await api.setSecret(p.key?.name ?? secretName(p), value, true);
            setDialog(null);
            reload();
          }}
        />
      )}
    </li>
  );
}

/** AI providers page: grouped list, key metadata, probe, fallback order. */
export function Providers() {
  const [list, setList] = useState<ProviderView[]>([]);
  const [probes, setProbes] = useState<Record<string, ProbeResult>>({});
  const [err, setErr] = useState<string | null>(null);

  const reload = useCallback(() => {
    api.listProviders().then(setList, (e) => setErr(errorText(e)));
  }, []);
  useEffect(reload, [reload]);

  const probe = async () => {
    try {
      const r = await api.probeProviders();
      setProbes(Object.fromEntries(r.map((x) => [x.id, x])));
    } catch (e) {
      setErr(errorText(e));
    }
  };

  const move = async (i: number, dir: -1 | 1) => {
    const ids = list.map((p) => p.id);
    const [id] = ids.splice(i, 1);
    if (id === undefined) return;
    ids.splice(i + dir, 0, id);
    try {
      await api.setProviderOrder(ids);
      reload();
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <section className="pv-shell" aria-label="AI providers">
      <h1 className="s-title">AI providers</h1>
      <button type="button" onClick={probe}>
        Probe local servers
      </button>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
      {GROUPS.map(([g, label]) => (
        <div key={g}>
          <h2 className="pv-group">{label}</h2>
          <ul>
            {list.map((p, i) =>
              p.group === g ? (
                <Row
                  key={p.id}
                  p={p}
                  probe={probes[p.id]}
                  index={i}
                  count={list.length}
                  onMove={(d) => move(i, d)}
                  reload={reload}
                />
              ) : null,
            )}
          </ul>
        </div>
      ))}
    </section>
  );
}
