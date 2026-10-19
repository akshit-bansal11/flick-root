import { type CSSProperties, useEffect, useRef } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { Logo } from "../tour/Logo";
import "./firstrun.css";

const CHIPS = [
  [">", "Commands"],
  ["!", "Apps"],
  ["/", "Find"],
  ["?", "Ask"],
] as const;

interface Props {
  /** Show the "no problem" note instead of the tour question. */
  note: boolean;
  onTour: () => void;
  onSkip: () => void;
  onContinue: () => void;
}

/** First screen of first run: logo, the four prefixes, and the tour question. */
export function Welcome({ note, onTour, onSkip, onContinue }: Props) {
  const primary = useRef<HTMLButtonElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: refocus when the screen switches to the note
  useEffect(() => primary.current?.focus(), [note]);

  if (note) {
    return (
      <main className="fr-shell fr-welcome">
        <p className="s-title fr-rise">
          No problem. You can take the tour any time from Settings &gt; General &gt; Take the tour.
        </p>
        <button type="button" ref={primary} className="fr-btn fr-primary" onClick={onContinue}>
          Continue
        </button>
      </main>
    );
  }
  return (
    <main className="fr-shell fr-welcome">
      <Logo size={96} animated />
      <h1 className="s-title fr-rise">Welcome to {APP_DISPLAY_NAME}</h1>
      <p className="s-help">One bar for apps, commands, files and questions.</p>
      <div className="fr-chips">
        {CHIPS.map(([k, label], i) => (
          <span key={k} className="fr-chip" style={{ "--i": i + 1 } as CSSProperties}>
            <kbd className="kbd">{k}</kbd> {label}
          </span>
        ))}
      </div>
      <p>Take a 2-minute interactive tour?</p>
      <div className="fr-actions">
        <button type="button" className="fr-btn" onClick={onSkip}>
          Not now
        </button>
        <button type="button" ref={primary} className="fr-btn fr-primary" onClick={onTour}>
          Take the tour
        </button>
      </div>
    </main>
  );
}
