export type Mode = "Command" | "Find" | "Query";
export const MODES: Mode[] = ["Command", "Find", "Query"];
export const PREFIX: Record<string, Mode> = { ">": "Command", "/": "Find", "?": "Query" };

export interface Item {
  title: string;
  sub: string;
  /** Lowercase search text. */
  keys: string;
}

export const ITEMS: Record<Mode, Item[]> = {
  Command: [
    { title: "Visual Studio Code", sub: "App", keys: "visual studio code vscode" },
    { title: "Calculator", sub: "App", keys: "calculator calc" },
    { title: "Windows Terminal", sub: "App", keys: "windows terminal wt" },
    { title: "2.5 kg to lb → 5.51 lb", sub: "Math and units", keys: "2.5 kg to lb" },
    { title: "Search Google for cats", sub: "Web shortcut", keys: "g cats" },
    { title: "Lock", sub: "System command", keys: "lock" },
    { title: "Sleep", sub: "System command", keys: "sleep" },
  ],
  Find: [
    { title: "notes.md", sub: "C:\\Users\\you\\Documents", keys: "notes md" },
    { title: "invoice-march.pdf", sub: "C:\\Users\\you\\Downloads", keys: "invoice march pdf" },
    { title: "my-app", sub: "D:\\projects", keys: "my app" },
  ],
  Query: [{ title: "Ask the provider you chose", sub: "Local or your own key", keys: "" }],
};

export const SCRIPT = ["code", "2.5 kg to lb", "g cats", "lock"];

/** Empty query lists everything; otherwise substring match. Query mode always shows its one row. */
export function filter(mode: Mode, q: string): Item[] {
  const s = q.trim().toLowerCase();
  if (mode === "Query") return ITEMS.Query;
  return ITEMS[mode].filter((i) => !s || i.keys.includes(s));
}
