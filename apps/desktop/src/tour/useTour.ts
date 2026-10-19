import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../settings/api";
import { fromEvent, matches, shortcut } from "../shortcuts";
import type { ThemeId } from "../theme";
import { ghostType, SAMPLE_PARAGRAPH } from "./demo";
import {
  ADVANCE_MS,
  acceleratorCaps,
  ghostInput,
  goalMet,
  isAccelerator,
  isLastStep,
  nextStep,
  prevStep,
  TOUR_STEPS,
  TOUR_THEMES,
  type TourEvent,
} from "./steps";
import { useDemo } from "./useDemo";

const warn = (err: unknown) => console.error(err);
const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta"]);

/** All tour behaviour: step state, hotkey pausing, goal detection, ghost typing. */
export function useTour(onDone: (completed: boolean) => void) {
  const [step, setStep] = useState(0);
  const [seen, setSeen] = useState<ReadonlySet<TourEvent>>(new Set());
  const [completed, setCompleted] = useState(false);
  const [theme, setTheme] = useState<ThemeId>("glass");
  const [themeSaved, setThemeSaved] = useState(false);
  const [hotkeys, setHotkeys] = useState({ launcher: "Alt+Space", explain: "Alt+Shift+Space" });
  const sample = useRef<HTMLParagraphElement>(null);
  const cancelGhost = useRef<(() => void) | null>(null);
  const finished = useRef(false);

  const mark = useCallback(
    (e: TourEvent) => setSeen((prev) => (prev.has(e) ? prev : new Set(prev).add(e))),
    [],
  );
  const demo = useDemo(mark);
  const { dispatch, reset, inputRef } = demo;
  const def = TOUR_STEPS[step];

  // Hotkeys are released while the tour runs so the demo sees the keys; always given back.
  useEffect(() => {
    api.setHotkeysPaused(true).catch(warn);
    api
      .getSettings()
      .then((s) => setHotkeys({ launcher: s.hotkeyLauncher, explain: s.hotkeyExplain }))
      .catch(warn);
    const resume = () => {
      api.setHotkeysPaused(false).catch(warn);
    };
    window.addEventListener("beforeunload", resume);
    return () => {
      window.removeEventListener("beforeunload", resume);
      resume();
    };
  }, []);

  const stopGhost = useCallback(() => {
    cancelGhost.current?.();
    cancelGhost.current = null;
  }, []);

  useEffect(() => stopGhost, [stopGhost]);

  // A new step starts from an empty bar and no progress.
  const goTo = useCallback(
    (n: number) => {
      stopGhost();
      reset();
      setSeen(new Set());
      setCompleted(false);
      setStep(n);
    },
    [reset, stopGhost],
  );

  const view = {
    mode: demo.state.mode,
    input: demo.state.input,
    attachment: demo.state.attachment !== null,
    seen,
  };
  const met = goalMet(step, view);
  useEffect(() => {
    if (!met) return;
    setCompleted(true);
    if (isLastStep(step)) return;
    const t = window.setTimeout(() => goTo(nextStep(step) ?? step), ADVANCE_MS);
    return () => window.clearTimeout(t);
  }, [met, step, goTo]);

  const finish = useCallback(
    (done: boolean) => {
      if (finished.current) return;
      finished.current = true;
      stopGhost();
      const after = () => onDone(done);
      if (done) api.setTutorialDone(true).catch(warn).then(after);
      else after();
    },
    [onDone, stopGhost],
  );

  const next = useCallback(() => {
    const n = nextStep(step);
    if (n === null) finish(true);
    else goTo(n);
  }, [step, finish, goTo]);
  const back = useCallback(() => goTo(prevStep(step)), [step, goTo]);
  const skip = useCallback(() => finish(false), [finish]);

  const capture = useCallback(() => {
    const text = window.getSelection()?.toString().trim() ?? "";
    dispatch({
      type: "captured",
      attachment: text
        ? {
            preview: text.slice(0, 2000),
            length: text.length,
            sourceApp: "Tour",
            blocked: false,
            reason: null,
          }
        : null,
    });
    if (text) mark("captured");
  }, [dispatch, mark]);

  const pickTheme = useCallback(
    (t: ThemeId) => {
      setTheme(t);
      setThemeSaved(false);
      mark("theme");
    },
    [mark],
  );

  const saveTheme = useCallback(() => {
    api
      .setAppearance({ theme })
      .then(() => setThemeSaved(true))
      .catch(warn);
  }, [theme]);

  const showMe = useCallback(() => {
    stopGhost();
    if (!def) return;
    switch (def.show) {
      case "hotkey":
        mark("hotkey");
        break;
      case "select": {
        const p = sample.current;
        const sel = window.getSelection();
        if (p && sel) {
          sel.removeAllRanges();
          const r = document.createRange();
          r.selectNodeContents(p);
          sel.addRange(r);
        }
        capture();
        break;
      }
      case "theme":
        pickTheme(TOUR_THEMES[(TOUR_THEMES.indexOf(theme) + 1) % TOUR_THEMES.length] ?? theme);
        break;
      case "type":
        if (!def.example) break;
        reset();
        inputRef.current?.focus();
        cancelGhost.current = ghostType(
          def.example,
          (typed) => dispatch({ type: "text", text: ghostInput(typed) }),
          () => {
            cancelGhost.current = null;
          },
        );
        break;
    }
  }, [def, stopGhost, mark, capture, pickTheme, theme, reset, inputRef, dispatch]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = fromEvent(e);
      if (matches(k, shortcut("tourNext").keys)) {
        e.preventDefault();
        next();
        return;
      }
      if (matches(k, shortcut("tourBack").keys)) {
        e.preventDefault();
        back();
        return;
      }
      if (def?.id === "open" && isAccelerator(hotkeys.launcher, e)) {
        e.preventDefault();
        mark("hotkey");
        return;
      }
      if (def?.id === "explain" && isAccelerator(hotkeys.explain, e)) {
        e.preventDefault();
        capture();
        return;
      }
      // The moment the user types, the ghost stops.
      if (!MODIFIERS.has(e.key)) stopGhost();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [def, hotkeys, next, back, mark, capture, stopGhost]);

  return {
    step,
    def,
    demo,
    theme,
    themeSaved,
    completed,
    open: step !== 0 || seen.has("hotkey"),
    keys: def?.hotkey ? acceleratorCaps(hotkeys[def.hotkey]) : (def?.keys ?? []),
    sample,
    samplePara: SAMPLE_PARAGRAPH,
    next,
    back,
    skip,
    showMe,
    pickTheme,
    saveTheme,
  };
}
