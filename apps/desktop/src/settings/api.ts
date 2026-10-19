// Typed bindings for every settings-window command. Tauri maps camelCase keys to Rust snake_case params.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  About,
  CaptureTest,
  ClearScope,
  EnginePatch,
  EngineProbe,
  EngineView,
  FeatureChanged,
  FeatureList,
  FeatureView,
  GeneralPatch,
  IntegrationPatch,
  IntegrationsView,
  IntegrationTarget,
  ModeToggle,
  ProbeResult,
  ProviderPatch,
  ProviderView,
  ReindexKind,
  SecretInfo,
  SettingsChanged,
  SettingsView,
  SpotifyState,
  TestResult,
} from "./types";

export const FEATURES_CHANGED = "features://changed";
export const SETTINGS_CHANGED = "settings://changed";

export const getSettings = () => invoke<SettingsView>("get_settings");
export const setGeneral = (patch: GeneralPatch) =>
  invoke<SettingsView>("set_general", {
    hotkeyLauncher: patch.hotkeyLauncher,
    hotkeyExplain: patch.hotkeyExplain,
    theme: patch.theme,
    launchAtStartup: patch.launchAtStartup,
  });
export const setRoots = (roots: string[]) => invoke<SettingsView>("set_roots", { roots });
export const setIncludeDrives = (includeDrives: boolean) =>
  invoke<SettingsView>("set_roots", { includeDrives });
export const clearHistory = (scope: ClearScope) => invoke<void>("clear_history", { scope });
export const reindex = (kind: ReindexKind) => invoke<void>("reindex", { kind });
export const getAbout = () => invoke<About>("get_about");
export const openLink = (url: string) => invoke<void>("open_link", { url });

export const listFeatures = () => invoke<FeatureList>("list_features");
export const setFeatureEnabled = (id: string, enabled: boolean) =>
  invoke<FeatureView>("set_feature_enabled", { id, enabled });
export const setFeatureOptions = (id: string, options: Record<string, unknown>) =>
  invoke<FeatureView>("set_feature_options", { id, options });
export const setModeEnabled = (mode: ModeToggle, enabled: boolean) =>
  invoke<void>("set_mode_enabled", { mode, enabled });

export const listProviders = () => invoke<ProviderView[]>("list_providers");
export const probeProviders = () => invoke<ProbeResult[]>("probe_providers");
export const setProvider = (id: string, patch: ProviderPatch) =>
  invoke<ProviderView>("set_provider", {
    id,
    enabled: patch.enabled,
    baseUrl: patch.baseUrl,
    model: patch.model,
    models: patch.models,
    effort: patch.effort,
  });
export const setProviderOrder = (ids: string[]) => invoke<void>("set_provider_order", { ids });

export const listEngines = () => invoke<EngineView[]>("list_engines");
export const probeDecisionEngines = () => invoke<EngineProbe[]>("probe_decision_engines");
export const setEngine = (id: string, patch: EnginePatch) =>
  invoke<EngineView[]>("set_engine", {
    id,
    enabled: patch.enabled,
    baseUrl: patch.baseUrl,
    candidates: patch.candidates,
    threshold: patch.threshold,
    timeoutMs: patch.timeoutMs,
  });

// The value goes in, never out: the result is metadata only.
export const setSecret = (name: string, value: string, acknowledged: boolean) =>
  invoke<SecretInfo>("set_secret", { name, value, acknowledged });
export const deleteSecret = (name: string) => invoke<void>("delete_secret", { name });
export const testSecret = (name: string) => invoke<TestResult>("test_secret", { name });
export const listSecrets = () => invoke<SecretInfo[]>("list_secrets");

export const onFeaturesChanged = (cb: (p: FeatureChanged) => void): Promise<UnlistenFn> =>
  listen<FeatureChanged>(FEATURES_CHANGED, (e) => cb(e.payload));
export const onSettingsChanged = (cb: (p: SettingsChanged) => void): Promise<UnlistenFn> =>
  listen<SettingsChanged>(SETTINGS_CHANGED, (e) => cb(e.payload));

export const SPOTIFY_STATE = "spotify://state";
export const onSpotifyState = (cb: (p: SpotifyState) => void): Promise<UnlistenFn> =>
  listen<SpotifyState>(SPOTIFY_STATE, (e) => cb(e.payload));
/** Opens the browser and waits for sign-in; rejects with code auth_cancelled, port_busy or network. */
export const connectSpotify = () => invoke<SpotifyState>("connect_spotify");
export const disconnectSpotify = () => invoke<void>("disconnect_spotify");

/** Checks one quick-capture target. A webhook target receives one test message. */
export const testCaptureTarget = (targetId: string) =>
  invoke<CaptureTest>("test_capture_target", { targetId });

export const getIntegrations = () => invoke<IntegrationsView>("get_integrations");
export const setIntegration = (target: IntegrationTarget, patch: IntegrationPatch) =>
  invoke<IntegrationsView>("set_integration", { target, patch });
