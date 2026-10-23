export type Mode = "none" | "Open" | "Commands" | "Calc" | "Ask" | "Projects";
export const PREFIX: Record<string, Exclude<Mode, "none">> = {
  "/": "Open",
  ">": "Commands",
  "=": "Calc",
  "?": "Ask",
  "@": "Projects",
};

export interface Item {
  title: string;
  sub: string;
  /** Lowercase search text. */
  keys: string;
}

const APPS: Item[] = [
  { title: "Visual Studio Code", sub: "App", keys: "visual studio code vscode" },
  { title: "Spotify", sub: "App", keys: "spotify spo" },
  { title: "Windows Terminal", sub: "App", keys: "windows terminal wt" },
];

const COMMANDS: Item[] = [
  { title: "Lock", sub: "System command", keys: "lock" },
  { title: "Sleep", sub: "System command", keys: "sleep" },
  { title: "Search Google for cats", sub: "Web shortcut", keys: "g cats" },
];

export const ITEMS: Record<Exclude<Mode, "none">, Item[]> = {
  Open: [
    ...APPS,
    { title: "report-q3.docx", sub: "C:\\Users\\you\\Documents", keys: "report q3 docx" },
    { title: "invoice-march.pdf", sub: "C:\\Users\\you\\Downloads", keys: "invoice march pdf" },
    { title: "projects", sub: "D:\\", keys: "projects" },
  ],
  Commands: COMMANDS,
  Calc: [
    { title: "2.5 kg to lb → 5.51 lb", sub: "Units", keys: "2.5 kg to lb" },
    { title: "250 usd in inr", sub: "Currency", keys: "250 usd in inr" },
  ],
  Ask: [{ title: "Ask the provider you chose", sub: "Local or your own key", keys: "" }],
  Projects: [
    { title: "flick", sub: "D:\\projects\\flick", keys: "flick" },
    { title: "notes-app", sub: "D:\\projects\\notes-app", keys: "notes app" },
  ],
};

/** Empty bar with no prefix: the prefix list. */
const PREFIX_LIST: Item[] = [
  { title: "/ Open", sub: "Apps, programs, files, folders", keys: "" },
  { title: "> Commands", sub: "System, web, media, ports, capture", keys: "" },
  { title: "= Calc", sub: "Math, units, currency, time zones", keys: "" },
  { title: "? Ask", sub: "Your AI provider", keys: "" },
  { title: "@ Projects", sub: "Folders you code in", keys: "" },
];

const FALLBACKS: Item[] = [
  { title: "Find files", sub: "Search instead", keys: "" },
  { title: "Search the web", sub: "Search instead", keys: "" },
  { title: "Ask", sub: "Search instead", keys: "" },
];

export const SCRIPT = ["spo", "/ report", "= 2.5 kg to lb", "> lock", "@ flick"];

/** Splits typed text into its leading prefix mode and the rest. */
export function parse(text: string): [Mode, string] {
  const m = PREFIX[text[0] ?? ""];
  return m ? [m, text.slice(1).trimStart()] : ["none", text];
}

/** Substring match. Ask always shows its one row; plain text ends with the fallback rows. */
export function filter(mode: Mode, q: string): Item[] {
  const s = q.trim().toLowerCase();
  if (mode === "Ask") return ITEMS.Ask;
  if (mode === "none") {
    if (!s) return PREFIX_LIST;
    return [...[...APPS, ...COMMANDS].filter((i) => i.keys.includes(s)), ...FALLBACKS];
  }
  return ITEMS[mode].filter((i) => !s || i.keys.includes(s));
}
