import { useCallback, useEffect, useRef, useState } from "react";
import * as ipc from "../ipc";
import { errorText } from "../settings/types";
import { fromEvent, parseChord, shortcut } from "../shortcuts";
import { type ActiveTheme, applyTheme } from "../theme";
import type { LaunchMode, Outcome } from "../types";
import { cancelsDefault } from "./keys";
import { type Effect, type Event, initialState, reduce, type State } from "./reducer";

/** Wires the pure reducer to the shell: runs effects through ipc.ts and feeds results back. */
export function useLauncher(
  inputRef: React.RefObject<HTMLInputElement | null>,
  initialShown?: ipc.ShownEvent | null,
) {
  const [state, setState] = useState<State>(() => initialState());
  const panelRef = useRef<HTMLDivElement>(null);
  const stateRef = useRef(state);
  const ask = useRef<{ token: number; id: string | null }>({ token: 0, id: null });
  const cancelled = useRef(new Set<number>());
  const lastHeight = useRef(0);
  const syncHeight = useRef((_force: boolean) => {});
  const copiedTimer = useRef<number | undefined>(undefined);
  /** The saved theme in use (last shown or applied); revert puts this back. */
  const saved = useRef<ActiveTheme | null>(null);

  const dispatch = useCallback((ev: Event): boolean => {
    const prev = stateRef.current;
    const r = reduce(prev, ev);
    stateRef.current = r.state;
    setState(r.state);
    for (const e of r.effects) run(e);
    return r.state !== prev || r.effects.length > 0;

    function outcome(commandId: string, p: Promise<Outcome>) {
      p.then((o) => dispatch({ type: "outcome", commandId, outcome: o })).catch((err: unknown) => {
        warn(err);
        dispatch({ type: "actionFailed", message: errorText(err) });
      });
    }
    function copy(p: Promise<void>) {
      p.then(() => dispatch({ type: "copyDone", ok: true })).catch((err: unknown) => {
        warn(err);
        dispatch({ type: "copyDone", ok: false });
      });
    }
    function run(e: Effect) {
      switch (e.type) {
        case "search":
          ipc
            .search({ seq: e.seq, mode: e.mode, text: e.text, sources: e.sources })
            .then((res) => dispatch({ type: "results", seq: res.seq, items: res.items }))
            .catch((err: unknown) => {
              warn(err);
              dispatch({ type: "searchFailed", seq: e.seq });
            });
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
          copy(ipc.copyPath(e.itemId));
          break;
        case "copyText":
          copy(ipc.copyText(e.text));
          break;
        case "clearCopied":
          window.clearTimeout(copiedTimer.current);
          copiedTimer.current = window.setTimeout(
            () => dispatch({ type: "copiedDone" }),
            e.afterMs,
          );
          break;
        case "hide":
          ipc.hideLauncher().catch(warn);
          break;
        case "openSettings":
          ipc.openSettings(e.section).catch(warn);
          break;
        case "ask":
          ipc
            .askStream(
              {
                text: e.text,
                attachment: e.attachment,
                effort: e.effort,
                thinkingMode: e.thinkingMode,
              },
              (event) => dispatch({ type: "ask", token: e.token, event }),
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
        case "loadThinkingModes":
          ipc
            .listThinkingModes()
            .then((r) => dispatch({ type: "thinkingModes", modes: r.modes, selected: r.selected }))
            .catch(warn);
          break;
        case "loadHome":
          ipc
            .homeItems()
            .then((r) => dispatch({ type: "homeLoaded", items: r.items, pins: r.pins }))
            .catch(warn);
          break;
        case "togglePin":
          ipc
            .togglePin(e.itemId)
            .then((r) => dispatch({ type: "homeLoaded", items: r.items, pins: r.pins }))
            .catch((err: unknown) => {
              warn(err);
              dispatch({ type: "actionFailed", message: errorText(err) });
            });
          break;
        case "loadThemes":
          ipc
            .listThemes()
            .then((themes) => dispatch({ type: "themesLoaded", themes }))
            .catch((err: unknown) => {
              warn(err);
              dispatch({ type: "themesLoaded", themes: [] });
            });
          break;
        case "previewTheme":
          // Repaint only: the list was loaded once, so an arrow key costs no IPC.
          applyTheme(document.documentElement, {
            id: e.theme.id,
            dark: e.theme.dark,
            roles: e.theme.roles,
          });
          break;
        case "revertTheme":
          applyTheme(document.documentElement, saved.current);
          break;
        case "applyTheme":
          ipc
            .setAppearance(e.patch)
            .then((view) => {
              saved.current = view.active;
              applyTheme(document.documentElement, view.active);
              dispatch({ type: "appearance", activeId: view.active?.id ?? null });
            })
            .catch((err: unknown) => {
              warn(err);
              applyTheme(document.documentElement, saved.current);
              dispatch({ type: "appearance", activeId: saved.current?.id ?? null });
              dispatch({ type: "actionFailed", message: errorText(err) });
            });
          break;
        case "webSearch":
          outcome("ui:web", ipc.webSearch(e.text));
          break;
        case "setThinkingMode":
          ipc.setThinkingMode(e.id).catch(warn);
          break;
      }
    }
  }, []);

  const first = useRef(initialShown);
  useEffect(() => {
    const onShown = (p: ipc.ShownEvent) => {
      saved.current = p.appearance?.active ?? null;
      applyTheme(document.documentElement, saved.current);
      dispatch({ type: "shown" });
      dispatch({ type: "appearance", activeId: saved.current?.id ?? null });
      if (p.sources) {
        dispatch({ type: "enabledSources", sources: p.sources, webTitle: p.webTitle ?? null });
      }
      inputRef.current?.focus();
      requestAnimationFrame(() => {
        syncHeight.current(true);
        ipc.reportShown(p.shownId).catch(warn);
      });
    };
    const unlisten = [
      ipc.onShown(onShown),
      ipc.onHidden(() => {
        lastHeight.current = 0;
        dispatch({ type: "hidden" });
      }),
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
    if (first.current) onShown(first.current);
    else dispatch({ type: "shown" });
    return () => {
      for (const u of unlisten) u.then((fn) => fn()).catch(warn);
    };
  }, [dispatch, inputRef]);

  // Window height follows the panel; unless forced (after show), only a changed height crosses IPC.
  useEffect(() => {
    const el = panelRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    syncHeight.current = (force) => {
      const h = Math.round(el.getBoundingClientRect().height);
      if (h === 0 || (!force && h === lastHeight.current)) return;
      lastHeight.current = h;
      ipc.resizeLauncher(h).catch(warn);
    };
    const ro = new ResizeObserver(() => syncHeight.current(false));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mode = stateRef.current.mode;
      const handled = dispatch({ type: "key", key: fromEvent(e) });
      if (cancelsDefault(e, mode, handled)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch]);

  const setText = useCallback((text: string) => dispatch({ type: "text", text }), [dispatch]);
  const activate = useCallback(
    (index: number, mods: { ctrl?: boolean; shift?: boolean } = {}) =>
      dispatch({ type: "activate", index, ...mods }),
    [dispatch],
  );

  const press = useCallback(
    (chord: string) => dispatch({ type: "key", key: parseChord(chord) }),
    [dispatch],
  );
  const toggleEffort = useCallback(() => press(shortcut("effort").keys), [press]);
  const selectTab = useCallback(
    (mode: LaunchMode) => {
      dispatch({ type: "tab", mode });
      inputRef.current?.focus();
    },
    [dispatch, inputRef],
  );
  const removeAttachment = useCallback(() => dispatch({ type: "removeAttachment" }), [dispatch]);
  const openSettings = useCallback((section: string) => ipc.openSettings(section).catch(warn), []);

  const openLink = useCallback((url: string) => ipc.openLink(url).catch(warn), []);

  return {
    state,
    panelRef,
    removeAttachment,
    selectTab,
    setText,
    activate,
    press,
    toggleEffort,
    openSettings,
    openLink,
  };
}

function warn(err: unknown) {
  console.error(err);
}
