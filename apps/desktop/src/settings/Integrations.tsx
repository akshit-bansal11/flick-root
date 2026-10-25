import { useCallback, useEffect, useState } from "react";
import { AddKeyDialog } from "./AddKeyDialog";
import * as api from "./api";
import { OnOff } from "./OnOff";
import { SettingRow } from "./Row";
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

/** Text field with its own save button, shown only once the value differs. */
function Field({ label, value, save, help }: FieldProps) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <div className="f-form">
      <label className="f-field">
        {label}
        <input className="s-input" value={v} onChange={(ev) => setV(ev.target.value)} />
      </label>
      {v !== value && (
        <div className="s-add">
          <button
            type="button"
            className="btn"
            aria-label={`Save ${label.toLowerCase()}`}
            onClick={() => save(v)}
          >
            save
          </button>
        </div>
      )}
      {help && <p className="s-prose">{help}</p>}
    </div>
  );
}

interface KeyButtonsProps {
  title: string;
  present: boolean;
  onAdd: () => void;
  onDelete: () => void;
}

/** Add or replace, plus delete when a key is stored. Buttons only; the row shows the status. */
function KeyButtons({ title, present, onAdd, onDelete }: KeyButtonsProps) {
  return (
    <>
      <button
        type="button"
        className="btn"
        aria-label={`${present ? "Replace" : "Add"} key for ${title}`}
        onClick={onAdd}
      >
        {present ? "replace" : "add key"}
      </button>
      {present && (
        <button
          type="button"
          className="btn"
          aria-label={`Delete key for ${title}`}
          onClick={onDelete}
        >
          delete
        </button>
      )}
    </>
  );
}

/** Runs test_capture_target and shows the outcome next to the button. */
function TestButton({ id, label, name }: { id: string; label: string; name: string }) {
  const [res, setRes] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try {
      setRes({ ok: true, text: (await api.testCaptureTarget(id)).message });
    } catch (e) {
      setRes({ ok: false, text: errorText(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button
        type="button"
        className="btn"
        aria-label={`${label} ${name}`}
        disabled={busy}
        onClick={run}
      >
        {label.toLowerCase()}
      </button>
      {res && (
        <span className={res.ok ? "s-help" : "s-error"} role={res.ok ? "status" : "alert"}>
          {res.text}
        </span>
      )}
    </>
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
    <>
      <SettingRow
        label="Spotify"
        value={connected ? (name ? `Connected as ${name}` : "connected") : undefined}
        hint="not connected"
        preview={{
          title: "# spotify",
          body: 'Works without connecting: "play X on spotify" opens the Spotify app. Connecting is optional and unlocks more Spotify features, using your own Spotify developer app. Playing, queueing and moving playback need Spotify Premium.',
        }}
      >
        <span className="s-add">
          {connected ? (
            <button
              type="button"
              className="btn"
              aria-label="Disconnect Spotify"
              onClick={disconnect}
            >
              disconnect
            </button>
          ) : (
            <button
              type="button"
              className="btn"
              aria-label={busy ? undefined : "Connect Spotify"}
              disabled={!v.client_id || busy}
              onClick={connect}
            >
              {busy ? "waiting for the browser…" : "connect"}
            </button>
          )}
        </span>
      </SettingRow>
      <ol className="s-prose">
        <li>Open the Spotify developer dashboard and create an app.</li>
        <li>
          Add this redirect URI:{" "}
          <code>{v.redirect_uri}</code>
        </li>
        <li>Copy the app's Client ID and paste it below.</li>
      </ol>
      <Field
        label="Spotify Client ID"
        value={v.client_id}
        save={(x) => apply("spotify", { clientId: x })}
      />
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}
    </>
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
      <section aria-label="Integrations">
        <h1 className="s-title">Integrations</h1>
        <p className="s-prose" role={err ? "alert" : undefined}>
          {err ?? "Loading…"}
        </p>
      </section>
    );
  }
  const notion = `integration.notion@${NOTION_HOST}`;
  const youtube = `integration.youtube@${YOUTUBE_HOST}`;
  const hookName = (name: string, host: string) => `integration.webhook.${name}@${host}`;

  return (
    <section aria-label="Integrations">
      <h1 className="s-title">Integrations</h1>
      <p className="s-desc s-prose">
        Nothing here is required. Each integration uses your own account and credentials, which are
        stored in Windows Credential Manager and sent only to their own host.
      </p>
      {err && (
        <p className="s-error" role="alert">
          {err}
        </p>
      )}

      <h2 className="sec"># media</h2>
      <Spotify v={v.spotify} apply={apply} />
      <SettingRow
        label="YouTube"
        value={v.youtube_key_present ? `Stored for ${YOUTUBE_HOST}` : undefined}
        hint="no key"
        preview={{
          title: "# youtube",
          body: 'Optional. With your own YouTube Data API key, "play X on youtube" opens the first result instead of the search page.',
        }}
      >
        <span className="s-add">
          <KeyButtons
            title="YouTube"
            present={v.youtube_key_present}
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
          <OnOff
            checked={v.youtube_use_api}
            label="Use the YouTube API"
            onChange={(on) => void apply("youtube", { enabled: on })}
          />
        </span>
      </SettingRow>

      <h2 className="sec"># quick capture</h2>
      <SettingRow
        label="Obsidian"
        value={v.obsidian_vault || undefined}
        hint="no vault"
        preview={{
          title: "# obsidian",
          body: "Appends quick-capture notes to the daily note in your vault.",
        }}
      >
        {v.obsidian_vault && (
          <span className="s-add">
            <TestButton id="obsidian" label="Test" name="Obsidian" />
          </span>
        )}
      </SettingRow>
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
      <SettingRow
        label="Markdown file"
        value={v.markdown_file || undefined}
        hint="no file"
        preview={{
          title: "# markdown file",
          body: "Notes are appended here. The file's folder must exist.",
        }}
      >
        {v.markdown_file && (
          <span className="s-add">
            <TestButton id="markdown" label="Test" name="Markdown file" />
          </span>
        )}
      </SettingRow>
      <Field
        label="Markdown file"
        value={v.markdown_file}
        save={(x) => apply("markdown", { file: x })}
        help="Notes are appended here. The file's folder must exist."
      />
      <SettingRow
        label="Notion"
        value={v.notion_token_present ? `Stored for ${NOTION_HOST}` : undefined}
        hint="no token"
        preview={{
          title: "# notion",
          body: "Create an internal integration in Notion, share your database with it, then add its token here.",
        }}
      >
        <span className="s-add">
          <KeyButtons
            title="Notion"
            present={v.notion_token_present}
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
          {v.notion_token_present && v.notion_database_id && (
            <TestButton id="notion" label="Test" name="Notion" />
          )}
        </span>
      </SettingRow>
      <Field
        label="Notion database ID"
        value={v.notion_database_id}
        save={(x) => apply("notion", { databaseId: x })}
      />

      <h2 className="sec"># webhooks</h2>
      <p className="s-prose">
        The webhook URL is a secret, so it is stored like a key. Enter the host first, then add the
        URL. "Send test" posts one test message to it.
      </p>
      {v.webhooks.map((w) => (
        <SettingRow
          key={w.name}
          label={w.name}
          value={w.url_present ? `URL stored for ${w.host}` : undefined}
          hint={`no URL (${w.host})`}
          preview={{
            title: `# webhook ${w.name}`,
            body: `Posts quick-capture notes to ${w.host}. The URL is stored like a key and never shown.`,
          }}
        >
          <span className="s-add">
            <button
              type="button"
              className="btn"
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
              {w.url_present ? "replace url" : "add url"}
            </button>
            {w.url_present && w.enabled && (
              <TestButton id={`webhook.${w.name}`} label="Send test" name={w.name} />
            )}
            <button
              type="button"
              className="btn"
              aria-label={`Remove webhook ${w.name}`}
              onClick={() => apply("webhook", { name: w.name, remove: true })}
            >
              remove
            </button>
            <OnOff
              checked={w.enabled}
              label={`Enable webhook ${w.name}`}
              onChange={(on) => void apply("webhook", { name: w.name, enabled: on })}
            />
          </span>
        </SettingRow>
      ))}
      <div className="f-form">
        <label className="f-field">
          Webhook name
          <input
            className="s-input"
            value={hook.name}
            onChange={(ev) => setHook({ ...hook, name: ev.target.value })}
          />
        </label>
        <label className="f-field">
          Webhook host
          <input
            className="s-input"
            value={hook.host}
            placeholder="hooks.example.com"
            onChange={(ev) => setHook({ ...hook, host: ev.target.value })}
          />
        </label>
        <div className="s-add">
          <button
            type="button"
            className="btn pri"
            disabled={!hook.name || !hook.host}
            onClick={async () => {
              await apply("webhook", hook);
              setHook({ name: "", host: "" });
            }}
          >
            add webhook
          </button>
        </div>
      </div>

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
