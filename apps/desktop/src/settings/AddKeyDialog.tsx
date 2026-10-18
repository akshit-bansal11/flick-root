import { useEffect, useId, useRef, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { errorText } from "./types";
import "./dialog.css";

interface Props {
  providerName: string;
  host: string;
  keyless: boolean;
  mode: "add" | "replace";
  onSave: (value: string, acknowledged: true) => Promise<void>;
  onCancel: () => void;
}

const FOCUSABLE = "input, button:not(:disabled)";

/** Modal that collects an API key. Presentational: the parent owns the IPC call. */
export function AddKeyDialog(props: Props) {
  if (props.keyless) return null;
  return <Dialog {...props} />;
}

function Dialog({ providerName, host, mode, onSave, onCancel }: Props) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const [value, setValue] = useState("");
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>("input")?.focus();
    return () => opener?.focus();
  }, []);

  const canSave = ack && value.trim() !== "" && !busy;

  const save = async () => {
    if (!canSave) return;
    setBusy(true);
    setError(null);
    try {
      await onSave(value, true);
      setValue("");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const cancel = () => {
    setValue("");
    onCancel();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    } else if (e.key === "Tab" && ref.current) {
      const els = [...ref.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const first = els[0];
      const last = els[els.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  };

  return (
    <div className="key-dialog-scrim">
      <div
        ref={ref}
        className="key-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <h2 id={titleId}>
          {mode === "replace" ? "Replace" : "Add"} key for {providerName}
        </h2>
        <p>
          {`Your key is stored in Windows Credential Manager and is only sent to ${host}. No storage is completely safe: malware on your account, a compromised extension or a breach at the provider could still expose it. Add it at your own risk. Before saving, set a spending or usage limit in your provider's dashboard, and use a key created only for ${APP_DISPLAY_NAME} so you can revoke it on its own.`}
        </p>
        <input
          type="password"
          aria-label="API key"
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <label className="key-dialog-ack">
          <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
          {"I understand the risk and have set a usage limit."}
        </label>
        {error && (
          <p role="alert" className="key-dialog-error">
            {error}
          </p>
        )}
        <div className="key-dialog-actions">
          <button type="button" onClick={cancel}>
            Cancel
          </button>
          <button type="button" disabled={!canSave} aria-busy={busy} onClick={save}>
            {busy ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
