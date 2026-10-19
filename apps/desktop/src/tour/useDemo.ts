import { useCallback, useEffect, useRef, useState } from "react";
import { type Effect, type Event, initialState, reduce, type State } from "../launcher/reducer";
import { fromEvent, parseChord, shortcut } from "../shortcuts";
import type { LaunchMode } from "../types";
import { createDemoRunner, tourEventOf } from "./demo";
import type { TourEvent } from "./steps";

const HANDLED = new Set(["ArrowDown", "ArrowUp", "Enter", "Escape", "Backspace", "Delete", "F1"]);
const TOAST_MS = 1800;

const fresh = (): State => reduce(initialState(), { type: "shown" }).state;

/**
 * The real reducer driven by the demo runner. Same wiring as useLauncher, but effects only
 * ever show toasts, and keys are read from the demo's own element, not the window.
 */
export function useDemo(onEvent: (e: TourEvent) => void) {
  const [state, setState] = useState<State>(fresh);
  const [toast, setToast] = useState<string | null>(null);
  const stateRef = useRef(state);
  const toastTimer = useRef<number | undefined>(undefined);
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;
  const inputRef = useRef<HTMLInputElement>(null);

  const showToast = useCallback((message: string) => {
    window.clearTimeout(toastTimer.current);
    setToast(message);
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  }, []);

  const [runner] = useState(() => {
    const host = { dispatch: (_ev: Event) => {}, toast: showToast };
    const r = createDemoRunner({ dispatch: (ev) => host.dispatch(ev), toast: showToast });
    return { r, host };
  });

  const dispatch = useCallback(
    (ev: Event): boolean => {
      const prev = stateRef.current;
      const res = reduce(prev, ev);
      stateRef.current = res.state;
      setState(res.state);
      if (ev.type === "key" && prev.mode !== "none" && res.state.mode === "none") {
        onEventRef.current("left");
      }
      for (const e of res.effects as Effect[]) {
        const seen = tourEventOf(e);
        if (seen) onEventRef.current(seen);
        runner.r.run(e);
      }
      return res.state !== prev || res.effects.length > 0;
    },
    [runner],
  );
  runner.host.dispatch = dispatch;

  useEffect(
    () => () => {
      runner.r.dispose();
      window.clearTimeout(toastTimer.current);
    },
    [runner],
  );

  /** Back to the empty bar, as if just opened. */
  const reset = useCallback(() => {
    runner.r.dispose();
    const s = fresh();
    stateRef.current = s;
    setState(s);
    setToast(null);
  }, [runner]);

  const onKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const handled = dispatch({ type: "key", key: fromEvent(e) });
      if (e.key === "Tab" || (handled && (HANDLED.has(e.key) || e.ctrlKey || e.altKey))) {
        e.preventDefault();
      }
    },
    [dispatch],
  );

  const press = useCallback(
    (chord: string) => dispatch({ type: "key", key: parseChord(chord) }),
    [dispatch],
  );

  return {
    state,
    toast,
    inputRef,
    dispatch,
    reset,
    onKeyDown,
    press,
    setText: useCallback((text: string) => dispatch({ type: "text", text }), [dispatch]),
    activate: useCallback(
      (index: number, mods: { ctrl?: boolean; shift?: boolean } = {}) =>
        dispatch({ type: "activate", index, ...mods }),
      [dispatch],
    ),
    toggleEffort: useCallback(() => press(shortcut("effort").keys), [press]),
    enterMode: useCallback(
      (mode: LaunchMode) => {
        dispatch({ type: "enterMode", mode });
        inputRef.current?.focus();
      },
      [dispatch],
    ),
    removeAttachment: useCallback(() => dispatch({ type: "removeAttachment" }), [dispatch]),
    openSettings: useCallback(
      () => showToast("The real launcher opens Settings here"),
      [showToast],
    ),
    openLink: useCallback(
      () => showToast("Links open in your browser in the real launcher"),
      [showToast],
    ),
  };
}
