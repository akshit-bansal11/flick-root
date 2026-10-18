// Mirrors the serde output of apps/desktop/src-tauri/src/commands/{settings,features,providers,secrets}.rs.
// No type here carries a secret value.

export type Theme = "system" | "dark" | "light";

export interface SettingsView {
  hotkeyLauncher: string;
  hotkeyExplain: string;
  theme: Theme;
  launchAtStartup: boolean;
  findEnabled: boolean;
  queryEnabled: boolean;
  roots: string[];
  extraRoots: string[];
}

export interface GeneralPatch {
  hotkeyLauncher?: string;
  hotkeyExplain?: string;
  theme?: Theme;
  launchAtStartup?: boolean;
}

export type ClearScope = "frecency" | "nl_cache" | "all";
export type ReindexKind = "apps" | "files";
export type ModeToggle = "find" | "query";

export interface About {
  version: string;
  license: string;
  links: { repo: string; docs: string; changelog: string; releases: string };
  logDir: string | null;
}

export type FeatureState =
  | "registered"
  | "starting"
  | "running"
  | "stopping"
  | "stopped"
  | "failed";

export interface Column {
  key: string;
  label: string;
}

export type FieldKind =
  | { type: "bool" }
  | { type: "number"; min: number; max: number; step: number }
  | { type: "text"; placeholder: string }
  | { type: "select"; options: string[] }
  | { type: "stringList" }
  | { type: "keyedList"; columns: Column[] }
  | { type: "path" }
  | { type: "secret"; nameTemplate: string };

export interface Field {
  key: string;
  label: string;
  help: string;
  kind: FieldKind;
  default: unknown;
  restart: boolean;
}

export interface FeatureView {
  id: string;
  title: string;
  versionTag: string;
  description: string;
  enabled: boolean;
  status: FeatureState;
  error: string | null;
  schema: { fields: Field[] };
  options: Record<string, unknown>;
}

export interface FeatureList {
  features: FeatureView[];
  onCount: number;
}

export interface FeatureChanged {
  id: string;
  enabled: boolean;
  status: FeatureState;
}

export interface SettingsChanged {
  section: string;
}

export type ProviderGroup = "local" | "free" | "own_key";
export type ProviderStatus = "off" | "key_needed" | "ready";
export type EffortSetting = "" | "fast" | "think";

// Rust SecretInfo has no rename_all: fields are snake_case on the wire.
export interface SecretInfo {
  name: string;
  present: boolean;
  last4: string | null;
  host: string;
  added_at: number | null;
  last_used: number | null;
}

// Rust ProviderView has no rename_all either: snake_case on the wire.
export interface ProviderView {
  id: string;
  title: string;
  group: ProviderGroup;
  enabled: boolean;
  base_url: string;
  model: string;
  key_required: boolean;
  key: SecretInfo | null;
  supports_effort: boolean;
  status: ProviderStatus;
}

export interface ProviderPatch {
  enabled?: boolean;
  baseUrl?: string;
  model?: string;
  effort?: EffortSetting;
}

export interface ProbeResult {
  id: string;
  reachable: boolean;
  models: string[];
  ms: number;
  error: string | null;
}

export interface TestResult {
  ok: boolean;
  status: number | null;
  ms: number;
}

/** Message of a Rust CommandError (`{ code, message }`), or a fallback. */
export function errorText(e: unknown): string {
  if (typeof e === "object" && e !== null && "message" in e) return String(e.message);
  return typeof e === "string" ? e : "Something went wrong";
}
