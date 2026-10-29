export type Mode = "none" | "Open" | "Commands" | "Calc" | "Ask";
export const PREFIX: Record<string, Exclude<Mode, "none">> = {
  "/": "Open",
  ">": "Commands",
  "=": "Calc",
  "?": "Ask",
};

export interface Item {
  title: string;
  /** Where it lives or what it does; shown on the right of the row and as a detail fact. */
  sub: string;
  /** Group header the row sits under. */
  group: string;
  /** Lowercase search text. */
  keys: string;
}

const APPS: Item[] = [
  { title: "Visual Studio Code", sub: "App", group: "apps", keys: "visual studio code vscode" },
  { title: "Spotify", sub: "App", group: "apps", keys: "spotify spo" },
  { title: "Windows Terminal", sub: "App", group: "apps", keys: "windows terminal wt" },
];

const COMMANDS: Item[] = [
  { title: "Lock", sub: "System command", group: "system", keys: "lock" },
  { title: "Sleep", sub: "System command", group: "system", keys: "sleep" },
  { title: "Search Google for cats", sub: "Web shortcut", group: "web", keys: "g cats" },
];

export const ITEMS: Record<Exclude<Mode, "none">, Item[]> = {
  Open: [
    ...APPS,
    {
      title: "report-q3.docx",
      sub: "C:\\Users\\you\\Documents",
      group: "files",
      keys: "report q3 docx",
    },
    {
      title: "invoice-march.pdf",
      sub: "C:\\Users\\you\\Downloads",
      group: "files",
      keys: "invoice march pdf",
    },
    { title: "projects", sub: "D:\\", group: "folders", keys: "projects" },
  ],
  Commands: COMMANDS,
  Calc: [
    { title: "2.5 kg to lb = 5.51 lb", sub: "Units", group: "results", keys: "2.5 kg to lb" },
    { title: "250 usd in inr", sub: "Currency", group: "results", keys: "250 usd in inr" },
  ],
  Ask: [
    {
      title: "Ask the provider you chose",
      sub: "Local or your own key",
      group: "ask",
      keys: "",
    },
  ],
};

/** Empty bar with no prefix: the prefix list. */
const PREFIX_LIST: Item[] = [
  { title: "/ Open", sub: "Apps, programs, files, folders", group: "prefixes", keys: "" },
  { title: "> Commands", sub: "System, web, media, ports, capture", group: "prefixes", keys: "" },
  { title: "= Calc", sub: "Math, units, currency, time zones", group: "prefixes", keys: "" },
  { title: "? Ask", sub: "Your AI provider", group: "prefixes", keys: "" },
];

const fallbacks = (q: string): Item[] => [
  { title: `Find files named “${q}”`, sub: "Search instead", group: "fallback", keys: "" },
  { title: `Search Google for “${q}”`, sub: "Search instead", group: "fallback", keys: "" },
  { title: `Ask your AI: “${q}”`, sub: "Search instead", group: "fallback", keys: "" },
];

export const SCRIPT = ["spo", "/ report", "= 2.5 kg to lb", "> lock"];

/** Tab strip order: `all` first, then one tab per prefix. */
export const TABS: { mode: Mode; label: string; key: string }[] = [
  { mode: "none", label: "all", key: "" },
  ...Object.entries(PREFIX).map(([key, mode]) => ({ mode, label: mode.toLowerCase(), key })),
];

/** Primary action word per mode. Every row also offers pin. */
export const PRIMARY: Record<Mode, string> = {
  none: "open",
  Open: "open",
  Commands: "run",
  Calc: "copy result",
  Ask: "ask",
};

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
    return [...[...APPS, ...COMMANDS].filter((i) => i.keys.includes(s)), ...fallbacks(q.trim())];
  }
  return ITEMS[mode].filter((i) => !s || i.keys.includes(s));
}
