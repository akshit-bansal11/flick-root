import { APP_DISPLAY_NAME } from "../brand";
import type { Mode } from "../types";

const KEYS: Record<Mode, string[]> = {
  command: ["↵ open", "Ctrl ↵ location", "Ctrl Shift ↵ admin", "Tab mode"],
  find: ["↵ open", "Ctrl ↵ location", "Ctrl C copy path", "Tab mode"],
  query: ["↵ ask", "Ctrl T effort", "Ctrl P provider", "Tab mode"],
};

export function Footer({ mode, canKill = false }: { mode: Mode; canKill?: boolean }) {
  const keys = canKill ? ["⌫ kill", ...KEYS[mode]] : KEYS[mode];
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
