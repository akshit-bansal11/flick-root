import { useRef } from "react";
import { Attachment } from "../query/Attachment";
import "../query/attachment.css";
import { QueryView } from "../query/QueryView";
import { Actions } from "./Actions";
import { Footer } from "./Footer";
import { InputBar } from "./InputBar";
import { Overlay, useOverlay } from "./Overlay";
import { HINTS_ID, hintId, LISTBOX_ID, optionId, PrefixHints, Results } from "./Results";
import { availableModes, DEFAULT_QUESTION } from "./reducer";
import { useLauncher } from "./useLauncher";

export function Launcher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const {
    state,
    removeAttachment,
    enterMode,
    setText,
    activate,
    press,
    toggleEffort,
    openSettings,
    openLink,
    panelRef,
  } = useLauncher(inputRef);

  const overlay = useOverlay();
  const q = state.query;
  const noProvider = q.errorCode === "no_provider";
  const none = state.mode === "none";
  const hints = availableModes(state.enabledModes);
  const hasList = none ? hints.length > 0 : state.items.length > 0;
  const activeId = !hasList ? undefined : none ? hintId(state.selected) : optionId(state.selected);

  return (
    <div ref={panelRef} className={state.visible ? "launcher show" : "launcher"}>
      <InputBar
        mode={state.mode}
        value={state.input}
        listId={none ? HINTS_ID : LISTBOX_ID}
        activeId={activeId}
        onChange={setText}
        inputRef={inputRef}
      />
      <div className="launcher-body">
        {overlay && <Overlay />}
        {state.mode === "query" && state.attachment && (
          <Attachment attachment={state.attachment} onRemove={removeAttachment} />
        )}
        {state.mode === "query" && state.notice && <p className="attach-notice">{state.notice}</p>}
        {state.mode === "query" ? (
          <QueryView
            question={q.status === "idle" ? "" : state.input || DEFAULT_QUESTION}
            provider={q.provider}
            effort={q.effort}
            text={q.text}
            error={noProvider ? null : q.error}
            noProvider={noProvider}
            modes={state.thinkingModes.modes}
            selectedMode={state.thinkingModes.selected}
            thinking={q.thinking}
            streaming={q.status === "streaming"}
            reasoning={q.reasoning}
            onToggleEffort={toggleEffort}
            onOpenSettings={() => openSettings("providers")}
            onOpenLink={openLink}
          />
        ) : none ? (
          <PrefixHints
            modes={hints}
            selected={state.selected}
            typed={state.input !== ""}
            onEnter={enterMode}
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
      <Footer
        mode={state.mode}
        canKill={state.armed && state.items[state.selected]?.kind === "process"}
      />
    </div>
  );
}
