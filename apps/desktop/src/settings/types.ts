// Mirrors the serde output of apps/desktop/src-tauri/src/commands/{settings,features,providers,secrets}.rs.
// No type here carries a secret value.

import type { Appearance } from "../theme";

/** Payload of get_appearance / set_appearance; mirrors AppearanceView in Rust. */
export type AppearanceView = Appearance;

/** Fields set_appearance accepts, each optional. */
export type AppearancePatch = Partial<Pick<AppearanceView, "scheme" | "darkTheme" | "lightTheme">>;

export interface SettingsView {
  hotkeyLauncher: string;
  hotkeyExplain: string;
  launchAtStartup: boolean;
  checkUpdates: boolean;
  findEnabled: boolean;
  queryEnabled: boolean;
  roots: string[];
  extraRoots: string[];
  includeDrives: boolean;
  scanPrograms: boolean;
  programSkip: string[];
  drives: DriveView[];
  programsCount: number;
  programsCapped: boolean;
}

/** One drive row; mirrors DriveView in Rust. */
export interface DriveView {
  root: string;
  kind: "fixed" | "removable";
  present: boolean;
}

export interface GeneralPatch {
  hotkeyLauncher?: string;
  hotkeyExplain?: string;
  launchAtStartup?: boolean;
  scanPrograms?: boolean;
  checkUpdates?: boolean;
}

export type ClearScope = "frecency" | "nl_cache" | "all";
export type ReindexKind = "apps" | "files";
export type ModeToggle = "find" | "query";

export interface About {
  version: string;
  license: string;
  links: { repo: string; docs: string; changelog: string; themes: string; releases: string };
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
  /** Extra models offered as thinking modes. */
  models: string[];
  key_required: boolean;
  key: SecretInfo | null;
  supports_effort: boolean;
  status: ProviderStatus;
}

export interface ProviderPatch {
  enabled?: boolean;
  baseUrl?: string;
  model?: string;
  models?: string[];
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

export type EngineProbeStatus = "reachable" | "not_running" | "no_key" | "no_provider";

// Rust EngineView / EngineProbe: snake_case on the wire.
export interface EngineView {
  id: string;
  title: string;
  enabled: boolean;
  base_url: string;
  key_host: string | null;
  needs_key: boolean;
  key_present: boolean;
  candidates: number;
  threshold: number;
  timeout_ms: number | null;
  order: number;
}

export interface EngineProbe {
  id: string;
  status: EngineProbeStatus;
  detail: string | null;
}

export interface EnginePatch {
  enabled?: boolean;
  baseUrl?: string;
  candidates?: number;
  threshold?: number;
  timeoutMs?: number;
}

// Rust IntegrationsView: snake_case on the wire. Flags only, never values.
export interface WebhookView {
  name: string;
  enabled: boolean;
  host: string;
  url_present: boolean;
}

export interface IntegrationsView {
  spotify: { client_id: string; redirect_uri: string; connected: boolean };
  youtube_use_api: boolean;
  youtube_key_present: boolean;
  obsidian_vault: string;
  obsidian_daily_folder: string;
  obsidian_daily_format: string;
  markdown_file: string;
  notion_database_id: string;
  notion_token_present: boolean;
  webhooks: WebhookView[];
}

/** Result of test_capture_target; failures reject with code not_configured or network. */
export interface CaptureTest {
  ok: boolean;
  message: string;
}

/** Payload of connect_spotify and the spotify://state event. */
export interface SpotifyState {
  connected: boolean;
  name: string | null;
}

export type IntegrationTarget =
  | "spotify"
  | "youtube"
  | "obsidian"
  | "markdown"
  | "notion"
  | "webhook";

/** Webhook URLs are never part of a patch: they go through setSecret. */
export interface IntegrationPatch {
  clientId?: string;
  enabled?: boolean;
  vault?: string;
  dailyFolder?: string;
  dailyFormat?: string;
  file?: string;
  databaseId?: string;
  name?: string;
  host?: string;
  newName?: string;
  remove?: boolean;
}
