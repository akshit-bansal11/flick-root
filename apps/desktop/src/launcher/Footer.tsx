import { APP_DISPLAY_NAME } from "../brand";
import type { LaunchMode } from "../types";

const KEYS: Record<LaunchMode, string[]> = {
  none: ["> commands", "! apps", "/ find", "? ask"],
  commands: ["↵ open", "Ctrl ↵ location", "Ctrl Shift ↵ admin"],
  run: ["↵ open", "Ctrl ↵ location", "Ctrl Shift ↵ admin"],
  find: ["↵ open", "Ctrl ↵ location", "Ctrl C copy path"],
  query: ["↵ ask", "Ctrl T effort", "Ctrl P provider"],
};

export function Footer({ mode, canKill = false }: { mode: LaunchMode; canKill?: boolean }) {
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
