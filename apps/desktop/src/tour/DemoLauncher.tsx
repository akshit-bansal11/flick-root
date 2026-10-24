import { useEffect, useLayoutEffect, useRef } from "react";
import { LauncherView } from "../launcher/LauncherView";
import { applyTheme, type ThemeView } from "../theme";
import type { useDemo } from "./useDemo";

interface Props {
  demo: ReturnType<typeof useDemo>;
  /** Resolved theme to preview; null keeps the tokens.css defaults. */
  theme: ThemeView | null;
  /** False before the hotkey step is done: the bar is dimmed and waits for the hotkey. */
  open: boolean;
}

/** The real launcher panel inside the tour. Themed on its own element; effects are fake. */
export function DemoLauncher({ demo, theme, open }: Props) {
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    applyTheme(el, theme);
  }, [theme]);

  // Focus moves into the bar as soon as it opens (and on first mount of a later step).
  useLayoutEffect(() => {
    if (open) demo.inputRef.current?.focus();
  }, [open, demo.inputRef]);

  // Keys typed in the bar reach the reducer through the wrapper; the window never sees them as launcher keys.
  useEffect(() => {
    const el = root.current;
    if (!el || !open) return;
    el.addEventListener("keydown", demo.onKeyDown);
    return () => el.removeEventListener("keydown", demo.onKeyDown);
  }, [open, demo.onKeyDown]);

  return (
    <div ref={root} className={open ? "demo" : "demo closed"} data-testid="demo">
      <LauncherView {...demo} />
      {demo.toast && (
        <p className="demo-toast" role="status">
          {demo.toast}
        </p>
      )}
    </div>
  );
}
