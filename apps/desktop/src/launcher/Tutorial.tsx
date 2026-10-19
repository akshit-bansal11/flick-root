import "./tutorial.css";
import { isLastStep, TUTORIAL_STEPS } from "./tutorialSteps";

interface Props {
  step: number;
  onNext: () => void;
  onBack: () => void;
  onSkip: () => void;
  onExample: (text: string) => void;
}

// Keep focus in the input: buttons react to click, but never take focus on press.
const keepFocus = (e: React.MouseEvent) => e.preventDefault();

/** Compact tour card under the input. Keyboard: Ctrl+Left / Ctrl+Right / Esc, handled by the reducer. */
export function Tutorial({ step, onNext, onBack, onSkip, onExample }: Props) {
  const s = TUTORIAL_STEPS[step];
  if (!s) return null;
  const last = isLastStep(step);
  return (
    <div role="dialog" aria-modal="false" aria-labelledby="tutorial-title" className="tutorial">
      <div className="tutorial-head">
        <h2 id="tutorial-title" className="tutorial-title">
          {s.title}
        </h2>
        <span className="tutorial-count">
          {step + 1} / {TUTORIAL_STEPS.length}
        </span>
      </div>
      <p className="tutorial-body">{s.body}</p>
      {(s.keys || s.example) && (
        <div className="tutorial-try">
          {s.keys?.map((k) => (
            <kbd key={k}>{k}</kbd>
          ))}
          {s.example && (
            <button
              type="button"
              className="tutorial-example"
              onMouseDown={keepFocus}
              onClick={() => s.example && onExample(s.example)}
            >
              Try: {s.example}
            </button>
          )}
        </div>
      )}
      <div className="tutorial-nav">
        <span className="tutorial-hint">Ctrl ← → move, Esc skips</span>
        <button type="button" onMouseDown={keepFocus} onClick={onSkip}>
          Skip
        </button>
        <button type="button" onMouseDown={keepFocus} onClick={onBack} disabled={step === 0}>
          Back
        </button>
        <button type="button" className="primary" onMouseDown={keepFocus} onClick={onNext}>
          {last ? "Finish" : "Next"}
        </button>
      </div>
    </div>
  );
}
