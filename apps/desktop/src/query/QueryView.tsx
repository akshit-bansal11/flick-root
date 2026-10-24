import type { Effort, ThinkingMode } from "../types";
import { EmptyState } from "./EmptyState";
import { Markdown } from "./Markdown";
import "./query.css";

interface Props {
  question: string;
  provider: string | null;
  model: string | null;
  effort: Effort;
  /** False when the active provider has no reasoning control; hides the effort row. */
  reasoning?: boolean;
  thinking?: boolean;
  streaming?: boolean;
  text: string;
  error: string | null;
  noProvider: boolean;
  /** Thinking modes Tab cycles through, in fallback order. */
  modes?: ThinkingMode[];
  selectedMode?: string | null;
  onToggleEffort: () => void;
  onOpenSettings: () => void;
  onOpenLink: (url: string) => void;
}

export function QueryView(p: Props) {
  const modes = p.modes ?? [];
  const mode = modes.find((m) => m.id === p.selectedMode) ?? modes[0];
  if (p.noProvider) return <EmptyState onOpenSettings={p.onOpenSettings} />;
  // EU AI Act Art. 50: the disclosure stays on screen with every generated answer.
  const who = [p.provider ?? mode?.providerLabel, p.model ?? mode?.model].filter(Boolean);
  const showEffort = p.reasoning !== false;
  const marker = ["AI answer", ...who, ...(showEffort ? [p.effort] : [])].join(" · ");
  return (
    <div className="split">
      <div className="left">
        {modes.length > 0 && (
          <>
            <div className="group" aria-hidden="true">
              # models, fallback order
            </div>
            <div role="listbox" aria-label="Models">
              {modes.map((m) => {
                const selected = m.id === mode?.id;
                return (
                  <div
                    key={m.id}
                    role="option"
                    tabIndex={-1}
                    aria-selected={selected}
                    className={selected ? "row sel" : "row"}
                  >
                    <span className="row-mark" aria-hidden="true">
                      {selected && "›"}
                    </span>
                    <span className="dot" aria-hidden="true" />
                    <span className="row-title">{`${m.providerLabel}/${m.model}`}</span>
                    {m.local && <span className="tag">local</span>}
                  </div>
                );
              })}
            </div>
          </>
        )}
        {showEffort && (
          <>
            <div className="group" aria-hidden="true">
              # effort
            </div>
            <button
              type="button"
              className="row effort"
              aria-pressed={p.effort === "think"}
              onClick={p.onToggleEffort}
            >
              <span className="row-mark" aria-hidden="true" />
              <span className="row-title">{p.effort === "think" ? "think" : "fast"}</span>
              <span className="tag">ctrl+t</span>
            </button>
          </>
        )}
      </div>
      <div className="right">
        {p.question && <div className="query-q">{p.question}</div>}
        {(p.text !== "" || p.streaming) && <p className="ai-mark">{marker}</p>}
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
    </div>
  );
}
