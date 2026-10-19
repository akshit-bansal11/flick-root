import type { Ref, RefObject } from "react";
import { Attachment } from "../query/Attachment";
import type { LaunchMode } from "../types";
import "../query/attachment.css";
import { QueryView } from "../query/QueryView";
import { Actions } from "./Actions";
import { Footer } from "./Footer";
import { InputBar } from "./InputBar";
import { HINTS_ID, hintId, LISTBOX_ID, optionId, PrefixHints, Results } from "./Results";
import { availableModes, DEFAULT_QUESTION, type State } from "./reducer";
import { ShortcutSheet } from "./ShortcutSheet";

/** What the launcher panel needs: a reducer state and the callbacks that feed events back. */
export interface LauncherViewProps {
  state: State;
  inputRef: RefObject<HTMLInputElement | null>;
  panelRef?: Ref<HTMLDivElement>;
  removeAttachment: () => void;
  enterMode: (mode: LaunchMode) => void;
  setText: (text: string) => void;
  activate: (index: number, mods?: { ctrl?: boolean; shift?: boolean }) => void;
  press: (chord: string) => void;
  toggleEffort: () => void;
  openSettings: (section: string) => void;
  openLink: (url: string) => void;
}

/** The launcher panel, presentational: the real window and the tour's demo both render it. */
export function LauncherView({
  state,
  inputRef,
  panelRef,
  removeAttachment,
  enterMode,
  setText,
  activate,
  press,
  toggleEffort,
  openSettings,
  openLink,
}: LauncherViewProps) {
  const q = state.query;
  const noProvider = q.errorCode === "no_provider";
  const none = state.mode === "none";
  const selected = state.items[state.selected];
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
        {state.mode === "query" && state.attachment && (
          <Attachment attachment={state.attachment} onRemove={removeAttachment} />
        )}
        {state.mode === "query" && state.notice && <p className="attach-notice">{state.notice}</p>}
        {state.sheet ? (
          <ShortcutSheet />
        ) : state.mode === "query" ? (
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
      {!state.sheet && state.mode === "find" && state.items.length > 0 && (
        <Actions onPress={press} />
      )}
      <Footer
        mode={state.mode}
        copied={state.copied}
        copyFailed={state.copyFailed}
        ctx={{
          armedProcess: state.armed && selected?.kind === "process",
          adminAction: selected?.actions.some((a) => a.type === "admin") ?? false,
          copyValue: selected?.actions.some((a) => a.type === "copyResult") ?? false,
          attachment: state.attachment !== null,
          answer: q.text !== "",
        }}
      />
    </div>
  );
}
