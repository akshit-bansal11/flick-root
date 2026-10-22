import { APP_DISPLAY_NAME } from "../brand";
import { Keycaps } from "../Keycaps";
import { type Condition, caps, inScope, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { useExit } from "../useMotion";
import { availableModes, MODES } from "./reducer";

interface Props {
  mode: LaunchMode;
  /** Modes the user can enter now; the empty-input hints list only these. */
  modes?: Exclude<LaunchMode, "none">[];
  /** Which conditional hints apply to the current selection. */
  ctx?: Partial<Record<Condition, boolean>>;
  /** Shows the short "Copied" confirmation. */
  copied?: boolean;
  /** Shows "Copy failed" instead. */
  copyFailed?: boolean;
}

export interface FootHint {
  id: string;
  caps: string[];
  label: string;
}

/** Footer hints per mode, straight from the shortcut table. */
export function footHints(
  mode: LaunchMode,
  ctx: Props["ctx"] = {},
  modes: Props["modes"] = availableModes(null),
): FootHint[] {
  if (mode === "none") {
    const prefixes = modes.map((m) => ({
      id: `go:${m}`,
      caps: [MODES[m].prefix],
      label: MODES[m].label.toLowerCase(),
    }));
    const sheet = { id: "sheet", caps: caps("sheet"), label: shortcut("sheet").foot ?? "" };
    return [...prefixes, sheet];
  }
  return inScope(mode)
    .filter((s) => s.foot && (!s.when || ctx[s.when]))
    .map((s) => ({ id: s.id, caps: caps(s.id), label: s.foot ?? "" }));
}

export function Footer({ mode, modes, ctx, copied = false, copyFailed = false }: Props) {
  const toast = useExit(copyFailed ? "Copy failed" : copied ? "Copied" : null);
  return (
    <div className="foot">
      <span className="brand">
        <span className="brand-dot" aria-hidden="true" />
        {APP_DISPLAY_NAME}
      </span>
      <span className="acts">
        {toast.shown && (
          <span
            className={toast.leaving ? "copied out" : "copied"}
            role={toast.leaving ? undefined : "status"}
            aria-hidden={toast.leaving || undefined}
            onAnimationEnd={toast.leaving ? toast.onEnd : undefined}
          >
            {toast.shown}
          </span>
        )}
        {footHints(mode, ctx, modes).map((h) => (
          <span key={h.id} className="act">
            <Keycaps caps={h.caps} /> {h.label}
          </span>
        ))}
      </span>
    </div>
  );
}
