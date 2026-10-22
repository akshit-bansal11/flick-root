// Mirrors crates/launcher-core/src/types.rs (serde camelCase).

// `search` takes source ids with the mode: "apps", "files", "features", or one feature id
// (math syscmd web media ports capture projects nlfind links frecency explain).
export type Mode = "command" | "find" | "query";

/** UI mode: "none" is the prefix-hint state; the others map onto a wire `Mode` and its sources. */
export type LaunchMode = "none" | "find" | "commands" | "calc" | "query" | "projects";

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
  thinkingMode?: string;
}

/** One selectable provider + model, from `list_thinking_modes`. */
export interface ThinkingMode {
  id: string;
  providerId: string;
  providerLabel: string;
  model: string;
  local: boolean;
}

export interface ThinkingModes {
  modes: ThinkingMode[];
  selected: string | null;
}

export type AskEvent =
  | { type: "started"; provider: string; model: string; reasoning?: boolean }
  | { type: "fallback"; from: string; to: string; reason: string }
  | { type: "thinking" }
  | { type: "delta"; text: string }
  | { type: "done"; provider: string }
  | { type: "error"; code: string; message: string };

export interface ShownPayload {
  shownId: number;
  reason: string;
}

/** `explain://captured` payload; `preview` is at most 2,000 characters. */
export interface ExplainCaptured {
  preview: string;
  length: number;
  source_app: string;
  blocked: boolean;
  reason?: string | null;
}
