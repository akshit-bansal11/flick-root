import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import { MaskedKey } from "./MaskedKey";
import { OnOff } from "./OnOff";
import { SettingRow } from "./Row";
import { errorText, type ProbeResult, type ProviderGroup, type ProviderView } from "./types";
import "./features.css";
import "./providers.css";

const GROUPS: [ProviderGroup, string][] = [
  ["local", "local"],
  ["free", "free gateways"],
  ["own_key", "your own key"],
];

function hostOf(url: string): string {
  try {
    return new URL(url).hostname;
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
  canUp: boolean;
  canDown: boolean;
  onMove: (dir: -1 | 1) => void;
  reload: () => void;
}

function Row({ p, probe, canUp, canDown, onMove, reload }: RowProps) {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState(p.base_url);
  const [model, setModel] = useState(p.model);
  useEffect(() => setUrl(p.base_url), [p.base_url]);
  useEffect(() => setModel(p.model), [p.model]);
  const [extra, setExtra] = useState(p.models.join(", "));
  useEffect(() => setExtra(p.models.join(", ")), [p.models]);
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

  const status = statusText(p, probe);
  const ok = p.key_required ? !!p.key : !!probe?.reachable;

  return (
    <>
      <SettingRow
        label={p.title}
        value={ok ? status : undefined}
        hint={ok ? undefined : status}
        preview={{
          title: `# ${p.title.toLowerCase()}`,
          body: `${status}. Enable it to use it as an answer source.`,
        }}
      >
        <span className="s-add">
          {p.key_required && !p.key && (
            <button
              type="button"
              className="btn"
              aria-label={`Add key for ${p.title}`}
              onClick={() => setDialog("add")}
            >
              add key
            </button>
          )}
          <button
            type="button"
            className="btn"
            aria-label={`Options for ${p.title}`}
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            options
          </button>
          <OnOff
            checked={p.enabled}
            label={`Enable ${p.title}`}
            onChange={(v) => void run(() => api.setProvider(p.id, { enabled: v }))}
          />
        </span>
      </SettingRow>
      {p.key && (
        <MaskedKey info={p.key} onReplace={() => setDialog("replace")} onDeleted={reload} />
      )}
      {open && (
        <div className="f-form">
          <label className="f-field">
            Base URL
            <input className="s-input" value={url} onChange={(e) => setUrl(e.target.value)} />
          </label>
          {hostChanged && p.key && (
            <p className="pv-warn" role="alert">
              Changing the host deletes the stored key for this provider when you save.
            </p>
          )}
          <label className="f-field">
            Model
            <input className="s-input" value={model} onChange={(e) => setModel(e.target.value)} />
          </label>
          <label className="f-field">
            Extra models
            <input
              className="s-input"
              value={extra}
              placeholder="comma-separated"
              onChange={(e) => setExtra(e.target.value)}
            />
          </label>
          <div className="s-add">
            <button
              type="button"
              className="btn"
              onClick={() =>
                run(() =>
                  api.setProvider(p.id, {
                    baseUrl: url,
                    model,
                    models: extra
                      .split(",")
                      .map((m) => m.trim())
                      .filter(Boolean),
                  }),
                )
              }
            >
              save options
            </button>
            <button
              type="button"
              className="btn"
              aria-label={`Move ${p.title} up`}
              disabled={!canUp}
              onClick={() => onMove(-1)}
            >
              ↑
            </button>
            <button
              type="button"
              className="btn"
              aria-label={`Move ${p.title} down`}
              disabled={!canDown}
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
          onSave={async (value, acknowledged) => {
            await api.setSecret(p.key?.name ?? secretName(p), value, acknowledged);
            setDialog(null);
            reload();
          }}
        />
      )}
    </>
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
    const g = list[i]?.group;
    let j = i + dir;
    while (list[j] && list[j]?.group !== g) j += dir;
    if (!list[j]) return;
    const ids = list.map((p) => p.id);
    const [id] = ids.splice(i, 1);
    if (id === undefined) return;
    ids.splice(j, 0, id);
    try {
      await api.setProviderOrder(ids);
      reload();
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <section aria-label="AI providers">
      <h1 className="s-title">AI providers</h1>
      <p className="s-desc s-prose">
        Choose where answers come from. Keys stay in Windows Credential Manager.
      </p>
      <button type="button" className="btn pri" onClick={probe}>
        probe local servers
      </button>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
      {GROUPS.map(([g, label]) => (
        <div key={g}>
          <h2 className="sec">{`# ${label}`}</h2>
          {list.map((p, i) =>
            p.group === g ? (
              <Row
                key={p.id}
                p={p}
                probe={probes[p.id]}
                canUp={list.slice(0, i).some((x) => x.group === g)}
                canDown={list.slice(i + 1).some((x) => x.group === g)}
                onMove={(d) => move(i, d)}
                reload={reload}
              />
            ) : null,
          )}
        </div>
      ))}
    </section>
  );
}
