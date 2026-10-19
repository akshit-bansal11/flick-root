import { useEffect, useRef } from "react";
import type { Effort, ThinkingMode } from "../types";
import { EmptyState } from "./EmptyState";
import { Markdown } from "./Markdown";
import "./query.css";

interface Props {
  question: string;
  provider: string | null;
  effort: Effort;
  /** False when the active provider has no reasoning control; hides the pill. */
  reasoning?: boolean;
  thinking?: boolean;
  streaming?: boolean;
  text: string;
  error: string | null;
  noProvider: boolean;
  /** Thinking modes Tab cycles through; none or one hides the cycling hint. */
  modes?: ThinkingMode[];
  selectedMode?: string | null;
  onToggleEffort: () => void;
  onOpenSettings: () => void;
  onOpenLink: (url: string) => void;
}

export function QueryView(p: Props) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.style.setProperty("--query-max", `${window.screen.availHeight * 0.6}px`);
  }, []);
  const modes = p.modes ?? [];
  const mode = modes.find((m) => m.id === p.selectedMode) ?? modes[0];
  if (p.noProvider) return <EmptyState onOpenSettings={p.onOpenSettings} />;
  return (
    <div ref={ref} className="query">
      {p.question && <div className="query-q">{p.question}</div>}
      <div className="query-chips">
        {mode ? (
          <span className="query-chip" aria-live="polite">
            {`${mode.providerLabel} · ${mode.model} · ${p.effort === "think" ? "Think" : "Fast"}${mode.local ? " · local" : ""}`}
          </span>
        ) : (
          p.provider && <span className="query-chip">{p.provider}</span>
        )}
        {p.reasoning !== false && (
          <button
            type="button"
            className={p.effort === "think" ? "query-pill on" : "query-pill"}
            aria-pressed={p.effort === "think"}
            onClick={p.onToggleEffort}
          >
            {p.effort === "think" ? "Think" : "Fast"}
          </button>
        )}
      </div>
      {p.thinking && !p.text && <div className="query-thinking">thinking…</div>}
      {p.text && (
        <div data-answer aria-live="polite" aria-busy={p.streaming}>
          <Markdown text={p.text} onOpenLink={p.onOpenLink} />
        </div>
      )}
      {p.error && (
        <div className="query-error" role="alert">
          {p.error}
        </div>
      )}
    </div>
  );
}
