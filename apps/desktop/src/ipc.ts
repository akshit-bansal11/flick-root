// The only module that talks to Tauri. Rust arg names are snake_case; Tauri expects camelCase keys here.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AskEvent, AskRequest, Mode, Outcome, SearchResponse, ShownPayload } from "./types";

export const search = (req: { seq: number; mode: Mode; text: string }) =>
  invoke<SearchResponse>("search", req);

export const launch = (itemId: string, admin: boolean) =>
  invoke<Outcome>("launch", { itemId, admin });

export const reveal = (itemId: string) => invoke<void>("reveal", { itemId });

export const copyPath = (itemId: string) => invoke<void>("copy_path", { itemId });

export const runSystemCommand = (commandId: string, confirmToken?: string) =>
  invoke<Outcome>("run_system_command", { commandId, confirmToken });

export function askStream(req: AskRequest, onEvent: (e: AskEvent) => void): Promise<string> {
  const onEventChannel = new Channel<AskEvent>();
  onEventChannel.onmessage = onEvent;
  return invoke<string>("ask_stream", { ...req, onEvent: onEventChannel });
}

export const cancelAsk = (askId: string) => invoke<void>("cancel_ask", { askId });

export const openLink = (url: string) => invoke<void>("open_link", { url });

export const hideLauncher = () => invoke<void>("hide_launcher");

export const resizeLauncher = (heightPx: number) => invoke<void>("resize_launcher", { heightPx });

export const reportShown = (shownId: number) => invoke<void>("report_shown", { shownId });

export const openSettings = (section?: string) => invoke<void>("open_settings", { section });

export const onShown = (cb: (p: ShownPayload) => void): Promise<UnlistenFn> =>
  listen<ShownPayload>("launcher://shown", (e) => cb(e.payload));

export const onHidden = (cb: () => void): Promise<UnlistenFn> =>
  listen("launcher://hidden", () => cb());
