import { useEffect, useRef } from "react";
import { resizeLauncher } from "../ipc";
import { InputBar } from "./InputBar";
import { Results } from "./Results";
import { useLauncher } from "./useLauncher";

export function Launcher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const { state, setText, activate } = useLauncher(inputRef);

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
      <Results items={state.items} selected={state.selected} onActivate={activate} />
    </div>
  );
}
