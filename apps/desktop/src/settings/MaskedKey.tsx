import { useState } from "react";
import * as api from "./api";
import "./features.css";
import { SettingRow } from "./Row";
import { errorText, type SecretInfo } from "./types";
import { useSwapFocus } from "./useSwapFocus";

interface Props {
  /** Provider name, so the repeated buttons are distinguishable to a screen reader. */
  title: string;
  info: SecretInfo;
  onReplace: () => void;
  onDeleted: () => void;
}

/** The sentence every key row shows: where it is stored and where it can be sent. */
export const keyHelp = (host: string) =>
  `Stored in Windows Credential Manager and only sent to ${host}. The key itself is never shown.`;

function relative(ts: number | null): string {
  if (ts === null) return "never";
  const s = Math.max(0, Date.now() / 1000 - (ts > 1e12 ? ts / 1000 : ts));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

/** Key metadata row: last4, host, last use, with Test / Replace / Delete. Never holds the key. */
export function MaskedKey({ title, info, onReplace, onDeleted }: Props) {
  const [confirming, setConfirming] = useState(false);
  const swap = useSwapFocus(confirming);
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
    swap.arm();
    setConfirming(false);
  };

  const ask = () => {
    swap.arm();
    setConfirming(true);
  };
  const keep = () => {
    swap.arm();
    setConfirming(false);
  };

  return (
    <>
      <SettingRow
        label="key"
        value={`•••• ${info.last4 ?? ""} · ${info.host} · last used ${relative(info.last_used)}`}
        help={keyHelp(info.host)}
        preview={{
          title: "# key",
          body: keyHelp(info.host),
        }}
      >
        <span className="s-add">
          <button type="button" className="btn" aria-label={`Test key for ${title}`} onClick={test}>
            test
          </button>
          <button
            type="button"
            className="btn"
            aria-label={`Replace key for ${title}`}
            onClick={onReplace}
          >
            replace
          </button>
          {confirming ? (
            <>
              <button
                type="button"
                className="btn s-btn-danger"
                ref={swap.whenOn}
                aria-label={`Confirm delete key for ${title}`}
                onClick={del}
              >
                confirm delete
              </button>
              <button
                type="button"
                className="btn"
                aria-label={`Keep key for ${title}`}
                onClick={keep}
              >
                keep
              </button>
            </>
          ) : (
            <button
              type="button"
              className="btn"
              ref={swap.whenOff}
              aria-label={`Delete key for ${title}`}
              onClick={ask}
            >
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
