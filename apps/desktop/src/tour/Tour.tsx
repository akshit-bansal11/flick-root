import type { CSSProperties } from "react";
import { Keycaps } from "../Keycaps";
import "../launcher/launcher.css";
import { MODES } from "../launcher/reducer";
import { DemoLauncher } from "./DemoLauncher";
import { Logo } from "./Logo";
import { isLastStep, TOUR_STEPS } from "./steps";
import { useTour } from "./useTour";
import "./tour.css";

interface Props {
  /** `completed` is true when the user reached the end, false when they skipped. */
  onDone: (completed: boolean) => void;
}

const SHOW_LABEL = { type: "Show me", hotkey: "Show me", select: "Show me", theme: "Next theme" };

/** Caps with a unique id each, since a chord list can repeat a cap ("Ctrl" twice). */
const capList = (keys: string[]) =>
  keys.map((k, i) => ({ k, id: `${k}#${keys.slice(0, i).filter((x) => x === k).length}` }));

/** The interactive tour: the real launcher panel with fake effects, plus a step card. */
export function Tour({ onDone }: Props) {
  const t = useTour(onDone);
  const def = t.def;
  if (!def) return null;
  const last = isLastStep(t.step);

  return (
    <section className="tour" aria-label="Interactive tour">
      <div className="tour-stage">
        <DemoLauncher demo={t.demo} theme={t.themeView} open={t.open} />
      </div>
      <div className="tour-card" key={def.id} aria-live="polite">
        <h2 className="tour-title">{def.title}</h2>
        <p className="tour-body">{def.body}</p>
        {t.keys.length > 0 && (
          <p className="tour-keys">
            {capList(t.keys).map((c, i) => (
              <span key={c.id} className="tour-cap" style={{ "--i": i } as CSSProperties}>
                <Keycaps caps={[c.k]} />
              </span>
            ))}
          </p>
        )}
        {def.id === "explain" && (
          <p ref={t.sample} className="tour-sample">
            {t.samplePara}
          </p>
        )}
        {def.id === "themes" && (
          <fieldset className="tour-themes">
            <legend className="sr-only">Themes</legend>
            {t.themes.map((v) => (
              <button
                key={v.id}
                type="button"
                className="tour-swatch"
                style={
                  {
                    "--sw-panel": v.roles.panel,
                    "--sw-text": v.roles.text,
                    "--sw-line": v.roles.line,
                  } as CSSProperties
                }
                aria-pressed={t.theme === v.id}
                onClick={() => t.pickTheme(v.id)}
              >
                {v.name}
              </button>
            ))}
            <button type="button" className="tour-link" onClick={t.saveTheme}>
              {t.themeSaved ? "Saved" : "Use this theme"}
            </button>
          </fieldset>
        )}
        {def.id === "done" && (
          <div className="tour-done">
            <Logo size={72} animated />
            <p className="tour-prefixes">
              {(Object.keys(MODES) as (keyof typeof MODES)[]).map((m) => (
                <span key={m}>
                  <Keycaps caps={[MODES[m].prefix]} /> {MODES[m].label}
                </span>
              ))}
            </p>
          </div>
        )}
        <div className="tour-actions">
          {def.show && (
            <button type="button" className="s-btn" onClick={t.showMe}>
              {SHOW_LABEL[def.show]}
            </button>
          )}
          {t.completed && !last && (
            <svg className="tour-check" viewBox="0 0 24 24" role="img" aria-label="Done">
              <circle cx="12" cy="12" r="10" />
              <path d="m7 12.5 3.2 3.2L17 9" />
            </svg>
          )}
        </div>
      </div>
      <nav className="tour-nav" aria-label="Tour">
        <ol className="tour-dots">
          {TOUR_STEPS.map((s, i) => (
            <li
              key={s.id}
              className={i === t.step ? "tour-dot on" : i < t.step ? "tour-dot past" : "tour-dot"}
              aria-current={i === t.step ? "step" : undefined}
            >
              <span className="sr-only">{`Step ${i + 1} of ${TOUR_STEPS.length}: ${s.title}`}</span>
            </li>
          ))}
        </ol>
        <div className="tour-buttons">
          {!last && (
            <button type="button" className="tour-link" onClick={t.skip}>
              Skip tour
            </button>
          )}
          <button type="button" className="s-btn" onClick={t.back} disabled={t.step === 0}>
            Back
          </button>
          <button type="button" className="s-btn s-btn-primary" onClick={t.next}>
            {last ? "Finish" : "Next"}
          </button>
        </div>
      </nav>
    </section>
  );
}
