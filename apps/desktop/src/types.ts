// Mirrors crates/launcher-core/src/types.rs and BSD sections 4 and 5 (serde camelCase).

export type Mode = "command" | "find" | "query";

export type ItemKind =
  | "app"
  | "command"
  | "file"
  | "folder"
  | "project"
  | "result"
  | "process"
  | "media"
  | "capture"
  | "choice";

export type IconRef =
  | { type: "file"; value: string }
  | { type: "dataUrl"; value: string }
  | { type: "glyph"; value: string };

export type ActionKind =
  | { type: "primary" }
  | { type: "admin" }
  | { type: "location" }
  | { type: "copyPath" }
  | { type: "copyResult" }
  | { type: "kill" }
  | { type: "custom"; value: string };

export interface Item {
  id: string;
  kind: ItemKind;
  title: string;
  subtitle: string;
  icon: IconRef | null;
  score: number;
  source: string;
  actions: ActionKind[];
  payload: unknown;
}

export type Outcome =
  | { type: "done" }
  | { type: "hideLauncher" }
  | { type: "copied" }
  | { type: "needsConfirm"; token: string; expiresMs: number }
  | { type: "showItems"; items: Item[] };

export type Effort = "fast" | "think";

export interface SearchResponse {
  seq: number;
  items: Item[];
  group: string | null;
}

export interface CommandError {
  code: string;
  message: string;
}

export interface AskRequest {
  text: string;
  attachment?: string;
  effort: Effort;
}

export type AskEvent =
  | { type: "started"; provider: string; model: string }
  | { type: "fallback"; from: string; to: string; reason: string }
  | { type: "thinking" }
  | { type: "delta"; text: string }
  | { type: "done"; provider: string }
  | { type: "error"; code: string; message: string };

export interface ShownPayload {
  shownId: number;
  reason: string;
}
