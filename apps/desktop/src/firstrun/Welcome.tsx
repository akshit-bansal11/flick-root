import { type CSSProperties, useEffect, useRef } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import { MODES } from "../launcher/reducer";
import { Logo } from "../tour/Logo";
import "./firstrun.css";

const CHIPS = Object.values(MODES);

interface Props {
  /** Show the "no problem" note instead of the tour question. */
  note: boolean;
  onTour: () => void;
  onSkip: () => void;
  onContinue: () => void;
}

/** Primary button that takes focus when it appears. */
function Primary({ onClick, children }: { onClick: () => void; children: string }) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => ref.current?.focus(), []);
  return (
    <button type="button" ref={ref} className="fr-btn fr-primary" onClick={onClick}>
      {children}
    </button>
  );
}

/** First screen of first run: logo, the four prefixes, and the tour question. */
export function Welcome({ note, onTour, onSkip, onContinue }: Props) {
  if (note) {
    return (
      <main className="fr-shell fr-welcome">
        <p className="s-title fr-rise">
          No problem. You can take the tour any time from Settings &gt; General &gt; Take the tour.
        </p>
        <Primary onClick={onContinue}>Continue</Primary>
      </main>
    );
  }
  return (
    <main className="fr-shell fr-welcome">
      <Logo size={96} animated />
      <h1 className="s-title fr-rise">Welcome to {APP_DISPLAY_NAME}</h1>
      <p className="s-help">One bar for apps, commands, files and questions.</p>
      <div className="fr-chips">
        {CHIPS.map((m, i) => (
          <span key={m.prefix} className="fr-chip" style={{ "--i": i + 1 } as CSSProperties}>
            <kbd className="kbd">{m.prefix}</kbd> {m.label}
          </span>
        ))}
      </div>
      <p>Take a 2-minute interactive tour?</p>
      <div className="fr-actions">
        <button type="button" className="fr-btn" onClick={onSkip}>
          Not now
        </button>
        <Primary onClick={onTour}>Take the tour</Primary>
      </div>
    </main>
  );
}
