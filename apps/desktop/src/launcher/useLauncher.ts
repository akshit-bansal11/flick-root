import { useCallback, useEffect, useRef, useState } from "react";
import * as ipc from "../ipc";
import type { Outcome } from "../types";
import { type Effect, type Event, initialState, reduce, type State } from "./reducer";

const HANDLED = new Set(["ArrowDown", "ArrowUp", "Enter", "Tab", "Escape"]);

/** Wires the pure reducer to the shell: runs effects through ipc.ts and feeds results back. */
export function useLauncher(inputRef: React.RefObject<HTMLInputElement | null>) {
  const [state, setState] = useState<State>(() => initialState());
  const stateRef = useRef(state);
  const askId = useRef<string | null>(null);

  const dispatch = useCallback((ev: Event): boolean => {
    const prev = stateRef.current;
    const r = reduce(prev, ev);
    stateRef.current = r.state;
    setState(r.state);
    for (const e of r.effects) run(e);
    return r.state !== prev || r.effects.length > 0;

    function outcome(commandId: string, p: Promise<Outcome>) {
      p.then((o) => dispatch({ type: "outcome", commandId, outcome: o })).catch(warn);
    }
    function run(e: Effect) {
      switch (e.type) {
        case "search":
          ipc
            .search({ seq: e.seq, mode: e.mode, text: e.text })
            .then((res) => dispatch({ type: "results", seq: res.seq, items: res.items }))
            .catch(warn);
          break;
        case "launch":
          outcome(e.itemId, ipc.launch(e.itemId, e.admin));
          break;
        case "confirm":
          outcome(e.commandId, ipc.runSystemCommand(e.commandId, e.token));
          break;
        case "reveal":
          ipc.reveal(e.itemId).catch(warn);
          break;
        case "copyPath":
          ipc.copyPath(e.itemId).catch(warn);
          break;
        case "hide":
          ipc.hideLauncher().catch(warn);
          break;
        case "openSettings":
          ipc.openSettings(e.section).catch(warn);
          break;
        case "ask":
          ipc
            .askStream({ text: e.text, effort: e.effort }, (event) =>
              dispatch({ type: "ask", event }),
            )
            .then((id) => {
              askId.current = id;
            })
            .catch(warn);
          break;
        case "cancelAsk":
          if (askId.current) ipc.cancelAsk(askId.current).catch(warn);
          break;
        // Resize is driven by the ResizeObserver in Launcher; provider switching is T2.8.
        case "resize":
        case "nextProvider":
          break;
      }
    }
  }, []);

  useEffect(() => {
    const unlisten = [
      ipc.onShown((p) => {
        dispatch({ type: "shown" });
        inputRef.current?.focus();
        requestAnimationFrame(() => {
          ipc.reportShown(p.shownId).catch(warn);
        });
      }),
      ipc.onHidden(() => dispatch({ type: "hidden" })),
    ];
    dispatch({ type: "shown" });
    return () => {
      for (const u of unlisten) u.then((fn) => fn()).catch(warn);
    };
  }, [dispatch, inputRef]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = inputRef.current;
      const handled = dispatch({
        type: "key",
        key: {
          key: e.key,
          ctrl: e.ctrlKey,
          shift: e.shiftKey,
          alt: e.altKey,
          hasSelection: !!el && el.selectionStart !== el.selectionEnd,
        },
      });
      if (handled && (HANDLED.has(e.key) || e.ctrlKey)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, inputRef]);

  const setText = useCallback((text: string) => dispatch({ type: "text", text }), [dispatch]);
  const activate = useCallback(
    (itemId: string) => {
      ipc
        .launch(itemId, false)
        .then((o) => dispatch({ type: "outcome", commandId: itemId, outcome: o }))
        .catch(warn);
    },
    [dispatch],
  );

  return { state, setText, activate };
}

function warn(err: unknown) {
  console.error(err);
}
