import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import * as api from "../settings/api";
import { comboFromEvent, conflictMessage, displayCombo, validateCombo } from "./hotkeyRules";

interface Props {
  initial: string;
  initialConflict: boolean;
  explainHotkey: string;
  onValid: (valid: boolean) => void;
}

type Status = { kind: "idle" } | { kind: "ok" } | { kind: "error"; text: string };

/** Step 1 body: capture a combo, validate it, try to register it. */
export function Hotkey({ initial, initialConflict, explainHotkey, onValid }: Props) {
  const [combo, setCombo] = useState(initial);
  const [capturing, setCapturing] = useState(initialConflict);
  const [status, setStatus] = useState<Status>(
    initialConflict ? { kind: "error", text: conflictMessage(initial) } : { kind: "idle" },
  );
  const [busy, setBusy] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    onValid(status.kind === "ok" || (status.kind === "idle" && !initialConflict));
  }, [status, initialConflict, onValid]);

  useEffect(() => {
    if (capturing) btn.current?.focus();
  }, [capturing]);

  const onKeyDown = async (e: KeyboardEvent) => {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      setCapturing(false);
      return;
    }
    const next = comboFromEvent(e);
    if (!next || busy) return;
    setCombo(next);
    const reason = validateCombo(next, explainHotkey);
    if (reason) {
      setStatus({ kind: "error", text: reason });
      return;
    }
    setBusy(true);
    try {
      await api.setGeneral({ hotkeyLauncher: next });
      setStatus({ kind: "ok" });
      setCapturing(false);
    } catch (err) {
      const code = typeof err === "object" && err !== null && "code" in err ? err.code : null;
      setStatus({
        kind: "error",
        text: code === "hotkey_conflict" ? conflictMessage(next) : "Could not set this shortcut.",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fr-hotkey">
      <span id="fr-hotkey-label" className="s-label">
        Launcher shortcut
      </span>
      <button
        ref={btn}
        type="button"
        className="fr-field"
        aria-labelledby="fr-hotkey-label"
        aria-describedby="fr-hotkey-status"
        data-capturing={capturing}
        onClick={() => setCapturing(true)}
        onKeyDown={onKeyDown}
        onBlur={() => setCapturing(false)}
      >
        {capturing ? "Press a shortcut…" : displayCombo(combo)}
      </button>
      <p
        id="fr-hotkey-status"
        className="fr-status"
        data-kind={status.kind}
        role="status"
        aria-live="polite"
      >
        {status.kind === "ok" ? "Shortcut works" : status.kind === "error" ? status.text : ""}
      </p>
    </div>
  );
}
