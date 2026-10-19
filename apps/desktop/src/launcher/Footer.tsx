import { APP_DISPLAY_NAME } from "../brand";
import { type Condition, capsText, inScope, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { useExit } from "../useMotion";
import { MODES } from "./reducer";

interface Props {
  mode: LaunchMode;
  /** Which conditional hints apply to the current selection. */
  ctx?: Partial<Record<Condition, boolean>>;
  /** Shows the short "Copied" confirmation. */
  copied?: boolean;
  /** Shows "Copy failed" instead. */
  copyFailed?: boolean;
}

/** Footer hints per mode, straight from the shortcut table. */
export function footHints(mode: LaunchMode, ctx: Props["ctx"] = {}): string[] {
  if (mode === "none") {
    const prefixes = (Object.keys(MODES) as Exclude<LaunchMode, "none">[]).map(
      (m) => `${MODES[m].prefix} ${MODES[m].label.toLowerCase()}`,
    );
    return [...prefixes, `${capsText("sheet")} ${shortcut("sheet").foot}`];
  }
  return inScope(mode)
    .filter((s) => s.foot && (!s.when || ctx[s.when]))
    .map((s) => `${capsText(s.id)} ${s.foot}`);
}

export function Footer({ mode, ctx, copied = false, copyFailed = false }: Props) {
  const toast = useExit(copyFailed ? "Copy failed" : copied ? "Copied" : null);
  return (
    <div className="foot">
      <span className="brand">
        <span className="brand-dot" aria-hidden="true" />
        {APP_DISPLAY_NAME}
      </span>
      <span className="keys">
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
        {footHints(mode, ctx).map((k) => (
          <span key={k}>{k}</span>
        ))}
      </span>
    </div>
  );
}
