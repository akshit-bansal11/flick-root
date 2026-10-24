import type { Ref, RefObject } from "react";
import { Attachment } from "../query/Attachment";
import type { LaunchMode } from "../types";
import "../query/attachment.css";
import { LiquidFilter } from "../query/LiquidFilter";
import { QueryView } from "../query/QueryView";
import { DetailPane } from "./DetailPane";
import { InputBar } from "./InputBar";
import { ModeTabs } from "./ModeTabs";
import { LISTBOX_ID, optionId, Results } from "./Results";
import { availableModes, DEFAULT_QUESTION, type State } from "./reducer";
import { ShortcutSheet } from "./ShortcutSheet";
import { countsOf, statusHints } from "./status";
import { StatusLine } from "./StatusLine";

/** What the launcher panel needs: a reducer state and the callbacks that feed events back. */
export interface LauncherViewProps {
  state: State;
  inputRef: RefObject<HTMLInputElement | null>;
  panelRef?: Ref<HTMLDivElement>;
  removeAttachment: () => void;
  selectTab: (mode: LaunchMode) => void;
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
  selectTab,
  setText,
  activate,
  press,
  toggleEffort,
  openSettings,
  openLink,
}: LauncherViewProps) {
  const q = state.query;
  const noProvider = q.errorCode === "no_provider";
  const selected = state.items[state.selected];
  const pathItem = selected?.id.startsWith("file:") ?? false;
  const revealable = selected?.actions.some((a) => a.type === "location") ?? false;
  const ctx = {
    pathItem,
    revealable,
    armedProcess: state.armed && selected?.kind === "process",
    adminAction: selected?.actions.some((a) => a.type === "admin") ?? false,
    copyValue: selected?.actions.some((a) => a.type === "copyResult") ?? false,
    attachment: state.attachment !== null,
    answer: q.text !== "",
  };
  const counts = state.mode === "query" ? [] : countsOf(state.items);
  const hints = availableModes(state.enabledSources);
  const activeId = state.items.length > 0 ? optionId(state.selected) : undefined;

  return (
    <div ref={panelRef} className={state.visible ? "launcher show" : "launcher"}>
      <LiquidFilter />
      <InputBar
        mode={state.mode}
        value={state.input}
        listId={LISTBOX_ID}
        activeId={activeId}
        onChange={setText}
        inputRef={inputRef}
      />
      <ModeTabs modes={hints} mode={state.mode} onSelect={selectTab} />
      {state.mode !== "query" && state.notice && (
        <p role="alert" className="launch-error">
          {state.notice}
        </p>
      )}
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
            model={q.model}
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
        ) : (
          <Results
            items={state.items}
            selected={state.selected}
            query={state.input}
            pending={state.pending}
            onActivate={activate}
            detail={selected && <DetailPane item={selected} ctx={ctx} onPress={press} />}
          />
        )}
      </div>
      <StatusLine
        status={statusHints(state.mode, ctx, counts)}
        copied={state.copied}
        copyFailed={state.copyFailed}
      />
    </div>
  );
}
