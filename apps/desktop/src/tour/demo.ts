// Sample data and a fake effect runner for the tour's demo launcher. Nothing here touches the
// system: effects become small toasts, searches filter the lists below, answers are canned.
import type { Effect, Event } from "../launcher/reducer";
import type { AskEvent, Item, ItemKind, Mode, SearchScope, ThinkingMode } from "../types";
import { GHOST_MS, type TourEvent } from "./steps";

const item = (
  id: string,
  kind: ItemKind,
  title: string,
  subtitle: string,
  extra: Partial<Item> = {},
): Item => ({
  id,
  kind,
  title,
  subtitle,
  icon: null,
  score: 1,
  source: "demo",
  actions: [{ type: "primary" }],
  payload: null,
  ...extra,
});

const program = (id: string, title: string, subtitle: string): Item =>
  item(id, "app", title, subtitle, {
    actions: [{ type: "primary" }, { type: "admin" }, { type: "location" }],
    payload: { program: true },
  });
const app = (id: string, title: string): Item =>
  item(id, "app", title, "Application", {
    actions: [{ type: "primary" }, { type: "admin" }, { type: "location" }],
  });
const file = (id: string, title: string, subtitle: string, kind: ItemKind = "file"): Item =>
  item(id, kind, title, subtitle, {
    actions: [{ type: "primary" }, { type: "location" }, { type: "copyPath" }],
  });

export const DEMO_APPS: readonly Item[] = [
  app("app:spotify", "Spotify"),
  app("app:chrome", "Chrome"),
  app("app:code", "VS Code"),
  app("app:notepad", "Notepad"),
  program("prog:notepadpp", "Notepad++", "C:\\Program Files\\Notepad++"),
  program("prog:codecs", "Codec Pack", "C:\\Program Files\\Codecs"),
];

export const DEMO_FILES: readonly Item[] = [
  file("file:report", "Quarterly report.pdf", "Documents"),
  file("file:reports", "Reports", "Documents", "folder"),
  file("file:notes", "Meeting notes.txt", "Documents"),
  file("file:photo", "Holiday photo.jpg", "Pictures"),
  file("file:budget", "Budget 2026.xlsx", "Documents"),
  file("file:proj", "flick", "Projects", "folder"),
];

const lockCommand = item("cmd:lock", "command", "Lock", "Lock this PC", {
  actions: [{ type: "primary" }],
});

const resultItem = (value: string, subtitle: string): Item =>
  item(`result:${value}`, "result", value, subtitle, {
    actions: [{ type: "copyResult" }],
    payload: { value },
  });

const proc = item("proc:chrome", "process", "chrome.exe", "Process", {
  actions: [{ type: "kill" }],
});

/** Canned results for the examples the steps use; anything else falls through to words. */
const CANNED: readonly [RegExp, () => Item[]][] = [
  [/^5\s*km\s*(to|in)\s*mi/i, () => [resultItem("3.107 mi", "5 km in miles")]],
  [/^2\s*\+\s*2$/, () => [resultItem("4", "2 + 2")]],
  [/^250\s*usd\s*(to|in)\s*inr/i, () => [resultItem("₹20,800", "250 USD in INR (sample rate)")]],
  [/^5pm\s*ist\s*(to|in)\s*pst/i, () => [resultItem("3:30 AM PST", "5 PM IST in PST")]],
  [/^lock/i, () => [lockCommand]],
  [/^(proc|port)/i, () => [proc]],
];

const matchText = (it: Item, text: string): boolean => {
  const q = text.trim().toLowerCase();
  return q === "" || it.title.toLowerCase().includes(q) || it.subtitle.toLowerCase().includes(q);
};

/** Items the demo shows for a search. */
export function demoSearch(mode: Mode, scope: SearchScope | undefined, text: string): Item[] {
  if (mode === "find") return DEMO_FILES.filter((i) => matchText(i, text));
  if (mode !== "command") return [];
  if (scope === "apps") return DEMO_APPS.filter((i) => matchText(i, text));
  const t = text.trim();
  for (const [re, make] of CANNED) if (re.test(t)) return make();
  return t === "" ? [] : DEMO_APPS.filter((i) => matchText(i, text)).slice(0, 3);
}

export const DEMO_MODES: ThinkingMode[] = [
  {
    id: "demo-local",
    providerId: "ollama",
    providerLabel: "Local",
    model: "llama3.2",
    local: true,
  },
  {
    id: "demo-claude",
    providerId: "claude",
    providerLabel: "Claude",
    model: "sonnet",
    local: false,
  },
];

/** The canned answer, streamed in chunks. */
export const DEMO_ANSWER = [
  "A **CIDR block** ",
  "writes an IP range as an address and a prefix length, ",
  "for example `10.0.0.0/24`. ",
  "The `/24` means the first 24 bits are the network, ",
  "which leaves 256 addresses.",
];

/** Selectable sample text for the Explain step. */
export const SAMPLE_PARAGRAPH =
  "A CIDR block writes an IP range as an address plus a prefix length. The prefix says how many leading bits name the network, and the rest name hosts inside it.";

export const ASK_CHUNK_MS = 70;

/** Types `text` into `emit` one character at a time; returns a cancel function. */
export function ghostType(
  text: string,
  emit: (typed: string) => void,
  onDone?: () => void,
  ms: number = GHOST_MS,
): () => void {
  let i = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = () => {
    i += 1;
    emit(text.slice(0, i));
    if (i < text.length) timer = setTimeout(tick, ms);
    else onDone?.();
  };
  if (text.length === 0) onDone?.();
  else timer = setTimeout(tick, ms);
  return () => clearTimeout(timer);
}

export interface DemoHost {
  dispatch: (ev: Event) => void;
  toast: (message: string) => void;
}

const byId = (id: string): Item | undefined =>
  [...DEMO_APPS, ...DEMO_FILES, lockCommand, proc].find((i) => i.id === id);

/** Runs reducer effects against the demo: toasts instead of actions, canned search and answers. */
export function createDemoRunner(host: DemoHost) {
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const asks = new Map<number, ReturnType<typeof setTimeout>[]>();
  const later = (fn: () => void, ms: number) => {
    const t = setTimeout(() => {
      timers.delete(t);
      fn();
    }, ms);
    timers.add(t);
    return t;
  };
  const name = (id: string) => byId(id)?.title ?? "it";

  function stream(token: number) {
    const ts: ReturnType<typeof setTimeout>[] = [];
    const at = (fn: () => void, ms: number) => ts.push(later(fn, ms));
    const ev = (event: AskEvent) => host.dispatch({ type: "ask", token, event });
    at(() => ev({ type: "started", provider: "Demo", model: "sample", reasoning: true }), 0);
    at(() => ev({ type: "thinking" }), ASK_CHUNK_MS);
    DEMO_ANSWER.forEach((text, i) => {
      at(() => ev({ type: "delta", text }), ASK_CHUNK_MS * (i + 2));
    });
    at(() => ev({ type: "done", provider: "Demo" }), ASK_CHUNK_MS * (DEMO_ANSWER.length + 2));
    asks.set(token, ts);
  }

  return {
    run(e: Effect) {
      switch (e.type) {
        case "search":
          later(
            () =>
              host.dispatch({
                type: "results",
                seq: e.seq,
                items: demoSearch(e.mode, e.scope, e.text),
              }),
            0,
          );
          break;
        case "launch":
          host.toast(
            e.itemId === "cmd:lock"
              ? "Would lock your PC"
              : byId(e.itemId)
                ? `Would ${e.admin ? "run as admin" : "open"} ${name(e.itemId)}`
                : "Would run it",
          );
          break;
        case "confirm":
        case "kill":
          host.toast(`Would end ${name("itemId" in e ? e.itemId : e.commandId)}`);
          break;
        case "reveal":
          host.toast("Would show it in Explorer");
          break;
        case "copyPath":
        case "copyText":
          // The reducer's own "Copied" status shows; the clipboard stays untouched.
          break;
        case "clearCopied":
          later(() => host.dispatch({ type: "copiedDone" }), e.afterMs);
          break;
        case "hide":
          host.toast("In the real launcher Esc hides it");
          break;
        case "openSettings":
          host.toast("The real launcher opens Settings here");
          break;
        case "ask":
          stream(e.token);
          break;
        case "cancelAsk":
          for (const t of asks.get(e.token) ?? []) clearTimeout(t);
          asks.delete(e.token);
          break;
        case "loadThinkingModes":
          later(
            () =>
              host.dispatch({
                type: "thinkingModes",
                modes: DEMO_MODES,
                selected: DEMO_MODES[0]?.id ?? null,
              }),
            0,
          );
          break;
        case "setThinkingMode":
          break;
      }
    },
    dispose() {
      for (const t of timers) clearTimeout(t);
      timers.clear();
      asks.clear();
    },
  };
}

/** The tour event an effect stands for, if any. */
export function tourEventOf(e: Effect): TourEvent | null {
  switch (e.type) {
    case "launch":
      return e.itemId.startsWith("file:")
        ? "openFile"
        : e.itemId.startsWith("app:") || e.itemId.startsWith("prog:")
          ? "launchApp"
          : null;
    case "reveal":
      return e.itemId.startsWith("file:") ? "openFile" : null;
    case "copyPath":
    case "copyText":
      return "copied";
    case "ask":
      return "askSent";
    case "setThinkingMode":
      return "modelSwitched";
    default:
      return null;
  }
}
