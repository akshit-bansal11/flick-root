import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { getSettings } from "../settings/api";
import { errorText } from "../settings/types";
import { Detect } from "./Detect";
import { Hotkey } from "./Hotkey";
import "../settings/settings.css";
import "./firstrun.css";

export interface FirstRunState {
  done: boolean;
  hotkey: string;
  hotkeyRegistered: boolean;
}
export const getFirstRun = () => invoke<FirstRunState>("get_first_run");
export const completeFirstRun = () => invoke<void>("complete_first_run");

export function FirstRun() {
  const [state, setState] = useState<FirstRunState | null>(null);
  const [explain, setExplain] = useState("");
  const [step, setStep] = useState<1 | 2>(1);
  const [valid, setValid] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getFirstRun(), getSettings()]).then(
      ([s, g]) => {
        setState(s);
        setExplain(g.hotkeyExplain);
      },
      (e: unknown) => setErr(errorText(e)),
    );
  }, []);

  const finish = () => completeFirstRun().catch((e: unknown) => setErr(errorText(e)));

  if (!state) {
    return (
      <main className="fr-shell">
        <p className="s-help" role={err ? "alert" : undefined}>
          {err ?? "Loading…"}
        </p>
      </main>
    );
  }

  return (
    <main className="fr-shell">
      <p className="fr-step">Step {step} of 2</p>
      <h1 className="s-title">
        {step === 1 ? `Pick your ${APP_DISPLAY_NAME} shortcut` : "Connect a local model"}
      </h1>
      {step === 1 ? (
        <Hotkey
          initial={state.hotkey}
          initialConflict={!state.hotkeyRegistered}
          explainHotkey={explain}
          onValid={setValid}
        />
      ) : (
        <Detect />
      )}
      {err && (
        <p className="fr-status" data-kind="error" role="alert">
          {err}
        </p>
      )}
      <div className="fr-actions">
        {step === 1 ? (
          <>
            {state.hotkeyRegistered && (
              <button type="button" className="fr-btn" onClick={() => setStep(2)}>
                Skip
              </button>
            )}
            <button
              type="button"
              className="fr-btn fr-primary"
              disabled={!valid}
              onClick={() => setStep(2)}
            >
              Next
            </button>
          </>
        ) : (
          <button type="button" className="fr-btn fr-primary" onClick={finish}>
            Finish
          </button>
        )}
      </div>
    </main>
  );
}
