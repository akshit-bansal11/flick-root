import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import {
  errorText,
  type IntegrationPatch,
  type IntegrationsView,
  type IntegrationTarget,
} from "./types";
import "./features.css";
import "./providers.css";

const NOTION_HOST = "api.notion.com";
const YOUTUBE_HOST = "www.googleapis.com";

interface Pending {
  title: string;
  host: string;
  name: string;
  replace: boolean;
}

type Apply = (target: IntegrationTarget, patch: IntegrationPatch) => Promise<void>;

interface FieldProps {
  label: string;
  value: string;
  save: (v: string) => void;
  help?: string;
}

/** Text field with its own Save button, shown only once the value differs. */
function Field({ label, value, save, help }: FieldProps) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <div className="pv-opts">
      <label>
        {label}
        <input value={v} onChange={(ev) => setV(ev.target.value)} />
      </label>
      {v !== value && (
        <button type="button" onClick={() => save(v)}>
          {`Save ${label.toLowerCase()}`}
        </button>
      )}
      {help && <p className="s-help">{help}</p>}
    </div>
  );
}

function KeyRow({
  present,
  host,
  onAdd,
  onDelete,
}: {
  present: boolean;
  host: string;
  onAdd: () => void;
  onDelete: () => void;
}) {
  return (
    <div className="pv-head">
      <span className="s-help">{present ? `Stored for ${host}` : "Not set"}</span>
      <button type="button" onClick={onAdd}>
        {present ? "Replace key" : "Add key"}
      </button>
      {present && (
        <button type="button" onClick={onDelete}>
          Delete key
        </button>
      )}
    </div>
  );
}

function Spotify({ v, apply }: { v: IntegrationsView["spotify"]; apply: Apply }) {
  const [connected, setConnected] = useState(v.connected);
  const [name, setName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => setConnected(v.connected), [v.connected]);
  useEffect(() => {
    const un = api.onSpotifyState((s) => {
      setConnected(s.connected);
      setName(s.name);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  const connect = async () => {
    setBusy(true);
    setErr(null);
    try {
      const s = await api.connectSpotify();
      setConnected(s.connected);
      setName(s.name);
    } catch (e) {
      setErr(
        typeof e === "object" && e !== null && "code" in e && e.code === "port_busy"
          ? `Port ${portOf(v.redirect_uri)} is in use — close the app using it and retry`
          : errorText(e),
      );
    } finally {
      setBusy(false);
    }
  };
  const disconnect = () =>
    api.disconnectSpotify().then(
      () => setName(null),
      (e) => setErr(errorText(e)),
    );
  return (
    <li className="pv-row">
      <div className="pv-head">
        <span className="pv-title">Spotify</span>
        <span className="pv-stat" data-ok={connected}>
          {connected ? (name ? `Connected as ${name}` : "connected") : "not connected"}
        </span>
      </div>
      <p className="s-help">
        Works without connecting: "play X on spotify" opens the Spotify app. Connecting is optional
        and unlocks more Spotify features, using your own Spotify developer app. Playing, queueing
        and moving playback need Spotify Premium.
      </p>
      <ol className="s-help">
        <li>Open the Spotify developer dashboard and create an app.</li>
        <li>
          Add this redirect URI: <code>{v.redirect_uri}</code>
        </li>
        <li>Copy the app's Client ID and paste it below.</li>
      </ol>
      <Field
        label="Spotify Client ID"
        value={v.client_id}
        save={(x) => apply("spotify", { clientId: x })}
      />
      <div className="pv-head">
        {connected ? (
          <button type="button" onClick={disconnect}>
            Disconnect Spotify
          </button>
        ) : (
          <button type="button" disabled={!v.client_id || busy} onClick={connect}>
            {busy ? "Waiting for the browser…" : "Connect Spotify"}
          </button>
        )}
      </div>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
    </li>
  );
}

function portOf(uri: string): string {
  return new URL(uri).port;
}

/** Settings page for integrations. Every one is optional and uses your own credentials. */
export function Integrations() {
  const [v, setV] = useState<IntegrationsView | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Pending | null>(null);
  const [hook, setHook] = useState({ name: "", host: "" });

  const reload = useCallback(() => {
    api.getIntegrations().then(setV, (e) => setErr(errorText(e)));
  }, []);
  useEffect(reload, [reload]);

  const apply: Apply = async (target, patch) => {
    try {
      setV(await api.setIntegration(target, patch));
      setErr(null);
    } catch (e) {
      setErr(errorText(e));
    }
  };
  const del = (name: string) => api.deleteSecret(name).then(reload, (e) => setErr(errorText(e)));

  if (!v) {
    return (
      <section className="pv-shell" aria-label="Integrations">
        <h1 className="s-title">Integrations</h1>
        <p className="s-help" role={err ? "alert" : undefined}>
          {err ?? "Loading…"}
        </p>
      </section>
    );
  }
  const notion = `integration.notion@${NOTION_HOST}`;
  const youtube = `integration.youtube@${YOUTUBE_HOST}`;
  const hookName = (name: string, host: string) => `integration.webhook.${name}@${host}`;

  return (
    <section className="pv-shell" aria-label="Integrations">
      <h1 className="s-title">Integrations</h1>
      <p className="s-desc">
        Nothing here is required. Each integration uses your own account and credentials, which are
        stored in Windows Credential Manager and sent only to their own host.
      </p>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}

      <h2 className="pv-group">Media</h2>
      <ul>
        <Spotify v={v.spotify} apply={apply} />
        <li className="pv-row">
          <div className="pv-head">
            <input
              type="checkbox"
              aria-label="Use the YouTube API"
              checked={v.youtube_use_api}
              onChange={(ev) => apply("youtube", { enabled: ev.target.checked })}
            />
            <span className="pv-title">YouTube</span>
          </div>
          <p className="s-help">
            Optional. With your own YouTube Data API key, "play X on youtube" opens the first result
            instead of the search page.
          </p>
          <KeyRow
            present={v.youtube_key_present}
            host={YOUTUBE_HOST}
            onAdd={() =>
              setDialog({
                title: "YouTube",
                host: YOUTUBE_HOST,
                name: youtube,
                replace: v.youtube_key_present,
              })
            }
            onDelete={() => del(youtube)}
          />
        </li>
      </ul>

      <h2 className="pv-group">Quick capture</h2>
      <ul>
        <li className="pv-row">
          <span className="pv-title">Obsidian</span>
          <Field
            label="Vault folder"
            value={v.obsidian_vault}
            save={(x) => apply("obsidian", { vault: x })}
            help="An existing folder on this PC."
          />
          <Field
            label="Daily note folder"
            value={v.obsidian_daily_folder}
            save={(x) => apply("obsidian", { dailyFolder: x })}
            help="Relative to the vault. Leave empty for the vault root."
          />
          <Field
            label="Daily note format"
            value={v.obsidian_daily_format}
            save={(x) => apply("obsidian", { dailyFormat: x })}
          />
        </li>
        <li className="pv-row">
          <span className="pv-title">Markdown file</span>
          <Field
            label="Markdown file"
            value={v.markdown_file}
            save={(x) => apply("markdown", { file: x })}
            help="Notes are appended here. The file's folder must exist."
          />
        </li>
        <li className="pv-row">
          <span className="pv-title">Notion</span>
          <p className="s-help">
            Create an internal integration in Notion, share your database with it, then add its
            token here.
          </p>
          <KeyRow
            present={v.notion_token_present}
            host={NOTION_HOST}
            onAdd={() =>
              setDialog({
                title: "Notion",
                host: NOTION_HOST,
                name: notion,
                replace: v.notion_token_present,
              })
            }
            onDelete={() => del(notion)}
          />
          <Field
            label="Notion database ID"
            value={v.notion_database_id}
            save={(x) => apply("notion", { databaseId: x })}
          />
        </li>
        <li className="pv-row">
          <span className="pv-title">Webhooks</span>
          <p className="s-help">
            The webhook URL is a secret, so it is stored like a key. Enter the host first, then add
            the URL.
          </p>
          <ul>
            {v.webhooks.map((w) => (
              <li key={w.name} className="pv-head">
                <input
                  type="checkbox"
                  aria-label={`Enable webhook ${w.name}`}
                  checked={w.enabled}
                  onChange={(ev) => apply("webhook", { name: w.name, enabled: ev.target.checked })}
                />
                <span className="pv-title">{w.name}</span>
                <span className="pv-stat" data-ok={w.url_present}>
                  {w.url_present ? `URL stored for ${w.host}` : `no URL (${w.host})`}
                </span>
                <button
                  type="button"
                  aria-label={`${w.url_present ? "Replace" : "Add"} URL for ${w.name}`}
                  onClick={() =>
                    setDialog({
                      title: `webhook ${w.name}`,
                      host: w.host,
                      name: hookName(w.name, w.host),
                      replace: w.url_present,
                    })
                  }
                >
                  {w.url_present ? "Replace URL" : "Add URL"}
                </button>
                <button
                  type="button"
                  aria-label={`Remove webhook ${w.name}`}
                  onClick={() => apply("webhook", { name: w.name, remove: true })}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <div className="pv-opts">
            <label>
              Webhook name
              <input
                value={hook.name}
                onChange={(ev) => setHook({ ...hook, name: ev.target.value })}
              />
            </label>
            <label>
              Webhook host
              <input
                value={hook.host}
                placeholder="hooks.example.com"
                onChange={(ev) => setHook({ ...hook, host: ev.target.value })}
              />
            </label>
            <button
              type="button"
              disabled={!hook.name || !hook.host}
              onClick={async () => {
                await apply("webhook", hook);
                setHook({ name: "", host: "" });
              }}
            >
              Add webhook
            </button>
          </div>
        </li>
      </ul>

      {dialog && (
        <AddKeyDialog
          providerName={dialog.title}
          host={dialog.host}
          keyless={false}
          mode={dialog.replace ? "replace" : "add"}
          onCancel={() => setDialog(null)}
          onSave={async (value, acknowledged) => {
            await api.setSecret(dialog.name, value, acknowledged);
            setDialog(null);
            reload();
          }}
        />
      )}
    </section>
  );
}
