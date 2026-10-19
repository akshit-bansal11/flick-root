import { useRef } from "react";
import { QueryView } from "../query/QueryView";
import { Actions } from "./Actions";
import { Footer } from "./Footer";
import { InputBar } from "./InputBar";
import { Overlay, useOverlay } from "./Overlay";
import { Results } from "./Results";
import { useLauncher } from "./useLauncher";

export function Launcher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const { state, setText, activate, press, toggleEffort, openSettings, openLink, hide } =
    useLauncher(inputRef);

  const overlay = useOverlay();
  const q = state.query;
  const noProvider = q.errorCode === "no_provider";

  return (
    <>
      <button
        type="button"
        className="backdrop"
        tabIndex={-1}
        aria-hidden="true"
        onMouseDown={hide}
      />
      <div className={state.visible ? "launcher show" : "launcher"}>
        <InputBar
          mode={state.mode}
          value={state.input}
          hasItems={state.items.length > 0}
          selected={state.selected}
          onChange={setText}
          inputRef={inputRef}
        />
        <div className="launcher-body">
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
            <Results
              items={state.items}
              selected={state.selected}
              query={state.input}
              onActivate={activate}
            />
          )}
        </div>
        {state.mode === "find" && state.items.length > 0 && <Actions onPress={press} />}
        <Footer mode={state.mode} />
      </div>
    </>
  );
}
