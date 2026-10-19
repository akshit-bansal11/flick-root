import { APP_DISPLAY_NAME } from "../brand";
import type { LaunchMode } from "../types";

const KEYS: Record<LaunchMode, string[]> = {
  none: ["> commands", "! apps", "/ find", "? ask", "F1 tutorial"],
  commands: ["↵ open", "Ctrl ↵ location", "Ctrl Shift ↵ admin"],
  run: ["↵ open", "Ctrl ↵ location", "Ctrl Shift ↵ admin"],
  find: ["↵ open", "Ctrl ↵ location", "Ctrl C copy path"],
  query: ["↵ ask", "Tab model", "Ctrl T think"],
};

interface Props {
  mode: LaunchMode;
  canKill?: boolean;
  /** Whether the selected item has an admin action (Commands mode shows the hint only then). */
  canAdmin?: boolean;
}

export function Footer({ mode, canKill = false, canAdmin = false }: Props) {
  const base = KEYS[mode].filter((k) => mode !== "commands" || canAdmin || !k.endsWith("admin"));
  const keys = canKill ? ["⌫ kill", ...base] : base;
  return (
    <div className="foot">
      <span className="brand">
        <span className="brand-dot" aria-hidden="true" />
        {APP_DISPLAY_NAME}
      </span>
      <span className="keys">
        {keys.map((k) => (
          <span key={k}>{k}</span>
        ))}
      </span>
    </div>
  );
}
