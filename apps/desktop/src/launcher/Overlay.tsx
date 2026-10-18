import { useCallback, useEffect, useState } from "react";
import "./overlay.css";

const SEEN_KEY = "prefixes-overlay-seen";
const MODIFIERS = new Set(["Shift", "Control", "Alt", "Meta"]);

function wasSeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "1";
  } catch {
    return false;
  }
}

/** One-time card explaining mode prefixes. Any key or click dismisses it for good. */
export function useOverlay() {
  const [open, setOpen] = useState(() => !wasSeen());

  const dismiss = useCallback(() => {
    setOpen(false);
    try {
      localStorage.setItem(SEEN_KEY, "1");
    } catch {
      // Storage unavailable: shown once per session only.
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (MODIFIERS.has(e.key)) return;
      // Capture phase, ahead of the launcher's key handler: the key only dismisses.
      e.preventDefault();
      e.stopImmediatePropagation();
      dismiss();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("click", dismiss);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("click", dismiss);
    };
  }, [open, dismiss]);

  return open;
}

export function Overlay() {
  return (
    <div role="note" aria-label="Mode prefixes" className="overlay">
      Tab changes mode. Type <kbd>&gt;</kbd> for Command, <kbd>/</kbd> for Find, <kbd>?</kbd> for
      Query.
    </div>
  );
}
