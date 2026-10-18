import { useEffect, useRef } from "react";
import { resizeLauncher } from "../ipc";
import { QueryView } from "../query/QueryView";
import { Actions } from "./Actions";
import { InputBar } from "./InputBar";
import { Overlay, useOverlay } from "./Overlay";
import { Results } from "./Results";
import { useLauncher } from "./useLauncher";

export function Launcher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const { state, setText, activate, press, toggleEffort, openSettings, openLink } =
    useLauncher(inputRef);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    let frame = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        resizeLauncher(Math.ceil(root.getBoundingClientRect().height)).catch(console.error);
      });
    });
    ro.observe(root);
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, []);

  const overlay = useOverlay();
  const q = state.query;
  const noProvider = q.errorCode === "no_provider";

  return (
    <div ref={rootRef} className={state.visible ? "launcher show" : "launcher"}>
      <InputBar
        mode={state.mode}
        value={state.input}
        hasItems={state.items.length > 0}
        selected={state.selected}
        onChange={setText}
        inputRef={inputRef}
      />
      {overlay && <Overlay />}
      {state.mode === "query" ? (
        <QueryView
          question={q.status === "idle" ? "" : state.input}
          provider={q.provider}
          effort={q.effort}
          text={q.text}
          error={noProvider ? null : q.error}
          noProvider={noProvider}
          thinking={q.thinking}
          streaming={q.status === "streaming"}
          reasoning={q.reasoning}
          onToggleEffort={toggleEffort}
          onOpenSettings={() => openSettings("providers")}
          onOpenLink={openLink}
        />
      ) : (
        <>
          <Results items={state.items} selected={state.selected} onActivate={activate} />
          {state.mode === "find" && state.items.length > 0 && <Actions onPress={press} />}
        </>
      )}
    </div>
  );
}
