import { useCallback, useEffect, useRef, useState } from "react";
import * as ipc from "../ipc";
import type { LaunchMode, Outcome } from "../types";
import { type Effect, type Event, initialState, reduce, type State } from "./reducer";

const HANDLED = new Set(["ArrowDown", "ArrowUp", "Enter", "Escape", "Backspace"]);

/** Wires the pure reducer to the shell: runs effects through ipc.ts and feeds results back. */
export function useLauncher(inputRef: React.RefObject<HTMLInputElement | null>) {
  const [state, setState] = useState<State>(() => initialState());
  const stateRef = useRef(state);
  const ask = useRef<{ token: number; id: string | null }>({ token: 0, id: null });
  const cancelled = useRef(new Set<number>());

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
            .search({ seq: e.seq, mode: e.mode, text: e.text, scope: e.scope })
            .then((res) => dispatch({ type: "results", seq: res.seq, items: res.items }))
            .catch(warn);
          break;
        case "launch":
          outcome(e.itemId, ipc.launch(e.itemId, e.admin));
          break;
        case "confirm":
          outcome(e.commandId, ipc.runSystemCommand(e.commandId, e.token, e.kill));
          break;
        case "kill":
          outcome(e.itemId, ipc.runSystemCommand(e.itemId, undefined, true));
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
            .askStream({ text: e.text, attachment: e.attachment, effort: e.effort }, (event) =>
              dispatch({ type: "ask", token: e.token, event }),
            )
            .then((id) => {
              if (cancelled.current.delete(e.token)) ipc.cancelAsk(id).catch(warn);
              else ask.current = { token: e.token, id };
            })
            .catch(warn);
          ask.current = { token: e.token, id: null };
          break;
        case "cancelAsk":
          // Id not back yet: cancel as soon as it arrives.
          if (ask.current.token === e.token && ask.current.id)
            ipc.cancelAsk(ask.current.id).catch(warn);
          else cancelled.current.add(e.token);
          break;
        // Provider switching is T2.8.
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
      ipc.onExplainCaptured((c) =>
        dispatch({
          type: "captured",
          attachment:
            c.length === 0 && !c.blocked
              ? null
              : {
                  preview: c.preview,
                  length: c.length,
                  sourceApp: c.source_app,
                  blocked: c.blocked,
                  reason: c.reason ?? null,
                },
        }),
      ),
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
      if (e.key === "Tab" || (handled && (HANDLED.has(e.key) || e.ctrlKey))) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, inputRef]);

  const setText = useCallback((text: string) => dispatch({ type: "text", text }), [dispatch]);
  const activate = useCallback(
    (index: number, mods: { ctrl?: boolean; shift?: boolean } = {}) =>
      dispatch({ type: "activate", index, ...mods }),
    [dispatch],
  );

  const press = useCallback(
    (key: string, ctrl: boolean) =>
      dispatch({ type: "key", key: { key, ctrl, shift: false, alt: false } }),
    [dispatch],
  );
  const toggleEffort = useCallback(() => press("t", true), [press]);
  const enterMode = useCallback(
    (mode: LaunchMode) => {
      dispatch({ type: "enterMode", mode });
      inputRef.current?.focus();
    },
    [dispatch, inputRef],
  );
  const removeAttachment = useCallback(() => dispatch({ type: "removeAttachment" }), [dispatch]);
  const openSettings = useCallback((section: string) => ipc.openSettings(section).catch(warn), []);

  const openLink = useCallback((url: string) => ipc.openLink(url).catch(warn), []);

  const hide = useCallback(() => ipc.hideLauncher().catch(warn), []);

  return {
    state,
    removeAttachment,
    enterMode,
    setText,
    activate,
    press,
    toggleEffort,
    openSettings,
    openLink,
    hide,
  };
}

function warn(err: unknown) {
  console.error(err);
}
