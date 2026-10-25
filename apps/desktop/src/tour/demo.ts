// Sample data and a fake effect runner for the tour's demo launcher. Nothing here touches the
// system: effects become small toasts, searches filter the lists below, answers are canned.
import { isProgram } from "../launcher/kinds";
import type { Effect, Event } from "../launcher/reducer";
import type { AskEvent, Item, ItemKind, Mode, ThinkingMode } from "../types";
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

export const DEMO_PROJECTS: readonly Item[] = [
  item("projects:flick", "project", "flick", "Projects"),
  item("projects:notes", "project", "notes-app", "Projects"),
];

/** Canned results for the examples the steps use, per source. */
type Canned = readonly [RegExp, () => Item[]][];
const CANNED_MATH: Canned = [
  [/^5\s*km\s*(to|in)\s*mi/i, () => [resultItem("3.107 mi", "5 km in miles")]],
  [/^2\s*\+\s*2$/, () => [resultItem("4", "2 + 2")]],
  [/^250\s*usd\s*(to|in)\s*inr/i, () => [resultItem("₹20,800", "250 USD in INR (sample rate)")]],
  [/^5pm\s*ist\s*(to|in)\s*pst/i, () => [resultItem("3:30 AM PST", "5 PM IST in PST")]],
];
const CANNED_SYSCMD: Canned = [
  [/^lock/i, () => [lockCommand]],
  [/^(proc|port)/i, () => [proc]],
];

const matchText = (it: Item, text: string): boolean => {
  const q = text.trim().toLowerCase();
  return q === "" || it.title.toLowerCase().includes(q) || it.subtitle.toLowerCase().includes(q);
};

const canned = (list: Canned, text: string): Item[] => {
  const t = text.trim();
  return list.find(([re]) => re.test(t))?.[1]() ?? [];
};

const DEMO_FEATURES: readonly string[] = ["math", "syscmd", "projects"];

/** Title the demo gives the first web shortcut, for the plain-text Web row. */
export const DEMO_WEB_TITLE = "Google";

/** Items the demo shows for a search, composed from `sources` in result order. */
export function demoSearch(mode: Mode, sources: readonly string[], text: string): Item[] {
  if (mode === "query") return [];
  // "features" stands for the demo's math, command and project sources.
  const has = (id: string) =>
    sources.includes(id) || (DEMO_FEATURES.includes(id) && sources.includes("features"));
  const match = (list: readonly Item[], pick: (i: Item) => boolean = () => true) =>
    list.filter((i) => pick(i) && matchText(i, text));
  return [
    ...(has("math") ? canned(CANNED_MATH, text) : []),
    ...(has("syscmd") ? canned(CANNED_SYSCMD, text) : []),
    ...(has("apps") ? match(DEMO_APPS, (i) => !isProgram(i)) : []),
    ...(has("apps") ? match(DEMO_APPS, isProgram) : []),
    ...(has("files") ? match(DEMO_FILES, (i) => i.kind === "file") : []),
    ...(has("files") ? match(DEMO_FILES, (i) => i.kind === "folder") : []),
    ...(has("projects") ? match(DEMO_PROJECTS) : []),
  ];
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
  [...DEMO_APPS, ...DEMO_FILES, ...DEMO_PROJECTS, lockCommand, proc].find((i) => i.id === id);

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
                items: demoSearch(e.mode, e.sources, e.text),
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
        case "webSearch":
          host.toast("Would search the web");
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
        case "loadThemes":
          // The demo has no theme list; the quick pick shows no rows.
          later(() => host.dispatch({ type: "themesLoaded", themes: [] }), 0);
          break;
        case "loadHome":
        case "setThinkingMode":
        case "previewTheme":
        case "revertTheme":
        case "applyTheme":
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
          : e.itemId.startsWith("cmd:")
            ? "runCommand"
            : e.itemId.startsWith("projects:")
              ? "openProject"
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
