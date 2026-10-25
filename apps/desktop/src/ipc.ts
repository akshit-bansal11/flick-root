// The only module that talks to Tauri.
// Contract: every Rust command takes FLAT parameters named exactly as the camelCase keys sent here
// (Tauri converts snake_case params), e.g. `search(seq, mode, text)` and
// `ask_stream(text, attachment, effort, on_event)`. Rust must not wrap them in a struct parameter.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppearancePatch } from "./settings/types";
import type { Appearance, ThemeView } from "./theme";
import type {
  AskEvent,
  AskRequest,
  ExplainCaptured,
  HomeView,
  Mode,
  Outcome,
  SearchResponse,
  ShownPayload,
  ThinkingModes,
} from "./types";

export const search = (req: {
  seq: number;
  mode: Mode;
  text: string;
  sources: readonly string[];
}) =>
  invoke<SearchResponse>("search", {
    seq: req.seq,
    mode: req.mode,
    text: req.text,
    sources: req.sources,
  });

export const launch = (itemId: string, admin: boolean) =>
  invoke<Outcome>("launch", { itemId, admin });

export const reveal = (itemId: string) => invoke<void>("reveal", { itemId });

export const copyPath = (itemId: string) => invoke<void>("copy_path", { itemId });

/** Copies text the user already sees (an answer or result value) through Rust; resolves once written. */
export const copyText = (text: string) => invoke<void>("copy_text", { text });

/** Closes the calling window (Settings). Needs core:window:allow-close in its capability. */
export const closeWindow = () => getCurrentWindow().close();

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
    thinkingMode: req.thinkingMode,
    onEvent: onEventChannel,
  });
}

export const listThinkingModes = () => invoke<ThinkingModes>("list_thinking_modes");

export const homeItems = () => invoke<HomeView>("home_items");

export const setThinkingMode = (id: string) => invoke<void>("set_thinking_mode", { id });

export const cancelAsk = (askId: string) => invoke<void>("cancel_ask", { askId });

export const openLink = (url: string) => invoke<void>("open_link", { url });

export const webSearch = (text: string) => invoke<Outcome>("web_search", { text });

export const hideLauncher = () => invoke<void>("hide_launcher");

export const resizeLauncher = (height: number) => invoke<void>("resize_launcher", { height });

export const reportShown = (shownId: number) => invoke<void>("report_shown", { shownId });

export const openSettings = (section?: string) => invoke<void>("open_settings", { section });

export type ShownEvent = ShownPayload & {
  appearance?: Appearance;
  /** Live source ids (apps, files, ask, running feature ids); absent from older shells. */
  sources?: string[];
  /** Title of the first web shortcut when the web feature runs. */
  webTitle?: string | null;
};

/** Every theme with its resolved roles; loaded once, never per keystroke. */
export const listThemes = () => invoke<ThemeView[]>("list_themes");

/** Saves appearance fields (the quick pick's apply); returns the appearance now in effect. */
export const setAppearance = (patch: AppearancePatch) =>
  invoke<Appearance>("set_appearance", patch);

export const onShown = (cb: (p: ShownEvent) => void): Promise<UnlistenFn> =>
  listen<ShownEvent>("launcher://shown", (e) => cb(e.payload));

export type TrayMenuEvent = {
  appVersion: string;
  hotkey: string;
  update: { version: string } | null;
  appearance?: Appearance;
};

export const onTrayMenu = (cb: (p: TrayMenuEvent) => void): Promise<UnlistenFn> =>
  listen<TrayMenuEvent>("tray://menu", (e) => cb(e.payload));

export const trayAction = (action: "open" | "settings" | "update" | "quit") =>
  invoke<void>("tray_action", { action });

export const onExplainCaptured = (cb: (c: ExplainCaptured) => void): Promise<UnlistenFn> =>
  listen<ExplainCaptured>("explain://captured", (e) => cb(e.payload));

export const onHidden = (cb: () => void): Promise<UnlistenFn> =>
  listen("launcher://hidden", () => cb());
