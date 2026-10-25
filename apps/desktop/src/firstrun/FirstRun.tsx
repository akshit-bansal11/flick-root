import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { getSettings } from "../settings/api";
import { errorText } from "../settings/types";
import { Tour } from "../tour/Tour";
import { Detect } from "./Detect";
import { Hotkey } from "./Hotkey";
import { Welcome } from "./Welcome";
import "../settings/settings.css";
import "./firstrun.css";

export interface FirstRunState {
  done: boolean;
  hotkey: string;
  hotkeyRegistered: boolean;
}
export const getFirstRun = () => invoke<FirstRunState>("get_first_run");
export const completeFirstRun = () => invoke<void>("complete_first_run");

const STEPS = ["welcome", "tour", "hotkey", "ask"];

export function FirstRun() {
  const [state, setState] = useState<FirstRunState | null>(null);
  const [explain, setExplain] = useState("");
  const [intro, setIntro] = useState<"welcome" | "tour" | "note" | null>("welcome");
  const [skipped, setSkipped] = useState(false);
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

  if (intro === "tour") return <Tour onDone={() => setIntro(null)} />;
  if (intro) {
    return (
      <Welcome
        note={intro === "note"}
        onTour={() => setIntro("tour")}
        onSkip={() => {
          setSkipped(true);
          setIntro("note");
        }}
        onContinue={() => setIntro(null)}
      />
    );
  }

  const at = STEPS.indexOf(step === 1 ? "hotkey" : "ask");

  return (
    <main className="fr-main">
      <div
        className="fr-prog"
        role="progressbar"
        aria-label="Setup progress"
        aria-valuemin={1}
        aria-valuemax={STEPS.length}
        aria-valuenow={at + 1}
      >
        {STEPS.map((s, i) => (
          <span key={s} data-done={i <= at} />
        ))}
      </div>
      <div className="fr-frame">
        <ol className="fr-steps">
          {STEPS.map((s, i) => (
            <li key={s} className="fr-st" aria-current={i === at ? "step" : undefined}>
              <span className="fr-mark" aria-hidden="true">{i < at ? "✓" : i === at ? "›" : ""}</span>
              {s}
              {s === "tour" && skipped && <span className="fr-skipped">skipped</span>}
            </li>
          ))}
        </ol>
        <div className="fr-body">
          <h1 className="s-title">
            {step === 1 ? `Pick your ${APP_DISPLAY_NAME} shortcut` : "How should Ask answer?"}
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
          <div className="fr-foot">
            <span className="fr-step">
              step {at + 1} of {STEPS.length}
            </span>
            <div className="fr-actions">
              {step === 1 ? (
                <>
                  {state.hotkeyRegistered && (
                    <button type="button" className="btn" onClick={() => setStep(2)}>
                      skip
                    </button>
                  )}
                  <button
                    type="button"
                    className="btn pri"
                    disabled={!valid}
                    onClick={() => setStep(2)}
                  >
                    next
                  </button>
                </>
              ) : (
                <button type="button" className="btn pri" onClick={finish}>
                  finish
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}
