import { useState } from "react";
import * as api from "./api";
import "./features.css";
import { SettingRow } from "./Row";
import { errorText, type SecretInfo } from "./types";

interface Props {
  info: SecretInfo;
  onReplace: () => void;
  onDeleted: () => void;
}

function relative(ts: number | null): string {
  if (ts === null) return "never";
  const s = Math.max(0, Date.now() / 1000 - (ts > 1e12 ? ts / 1000 : ts));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

/** Key metadata row: last4, host, last use, with Test / Replace / Delete. Never holds the key. */
export function MaskedKey({ info, onReplace, onDeleted }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const test = async () => {
    setResult("Testing…");
    try {
      const r = await api.testSecret(info.name);
      setResult(r.ok ? `Works (${r.ms} ms)` : `Failed${r.status ? ` (HTTP ${r.status})` : ""}`);
    } catch (e) {
      setResult(errorText(e));
    }
  };

  const del = async () => {
    try {
      await api.deleteSecret(info.name);
      onDeleted();
    } catch (e) {
      setResult(errorText(e));
    }
    setConfirming(false);
  };

  return (
    <>
      <SettingRow
        label="key"
        value={`•••• ${info.last4 ?? ""} · ${info.host} · last used ${relative(info.last_used)}`}
        preview={{
          title: "# key",
          body: `Stored in Windows Credential Manager and only sent to ${info.host}. The key itself is never shown.`,
        }}
      >
        <span className="s-add">
          <button type="button" className="btn" onClick={test}>
            test
          </button>
          <button type="button" className="btn" onClick={onReplace}>
            replace
          </button>
          {confirming ? (
            <>
              <button type="button" className="btn s-btn-danger" onClick={del}>
                confirm delete
              </button>
              <button type="button" className="btn" onClick={() => setConfirming(false)}>
                keep
              </button>
            </>
          ) : (
            <button type="button" className="btn" onClick={() => setConfirming(true)}>
              delete
            </button>
          )}
        </span>
      </SettingRow>
      {result && (
        <div role="status" className="s-help s-editor">
          {result}
        </div>
      )}
    </>
  );
}
