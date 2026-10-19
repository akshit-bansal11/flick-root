// The only module that talks to Tauri.
// Contract: every Rust command takes FLAT parameters named exactly as the camelCase keys sent here
// (Tauri converts snake_case params), e.g. `search(seq, mode, text)` and
// `ask_stream(text, attachment, effort, on_event)`. Rust must not wrap them in a struct parameter.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AskEvent,
  AskRequest,
  ExplainCaptured,
  Mode,
  Outcome,
  SearchResponse,
  ShownPayload,
} from "./types";

export const search = (req: { seq: number; mode: Mode; text: string }) =>
  invoke<SearchResponse>("search", { seq: req.seq, mode: req.mode, text: req.text });

export const launch = (itemId: string, admin: boolean) =>
  invoke<Outcome>("launch", { itemId, admin });

export const reveal = (itemId: string) => invoke<void>("reveal", { itemId });

export const copyPath = (itemId: string) => invoke<void>("copy_path", { itemId });

export const runSystemCommand = (commandId: string, confirmToken?: string, kill = false) =>
  invoke<Outcome>("run_system_command", { commandId, confirmToken, kill });

export function askStream(req: AskRequest, onEvent: (e: AskEvent) => void): Promise<string> {
  // One Channel per call, so only this ask's events reach onEvent; the caller tags them with its token.
  const onEventChannel = new Channel<AskEvent>();
  onEventChannel.onmessage = onEvent;
  return invoke<string>("ask_stream", {
    text: req.text,
    attachment: req.attachment,
    effort: req.effort,
    onEvent: onEventChannel,
  });
}

export const cancelAsk = (askId: string) => invoke<void>("cancel_ask", { askId });

export const openLink = (url: string) => invoke<void>("open_link", { url });

export const hideLauncher = () => invoke<void>("hide_launcher");

export const reportShown = (shownId: number) => invoke<void>("report_shown", { shownId });

export const openSettings = (section?: string) => invoke<void>("open_settings", { section });

export const onShown = (cb: (p: ShownPayload) => void): Promise<UnlistenFn> =>
  listen<ShownPayload>("launcher://shown", (e) => cb(e.payload));

export const onExplainCaptured = (cb: (c: ExplainCaptured) => void): Promise<UnlistenFn> =>
  listen<ExplainCaptured>("explain://captured", (e) => cb(e.payload));

export const onHidden = (cb: () => void): Promise<UnlistenFn> =>
  listen("launcher://hidden", () => cb());
