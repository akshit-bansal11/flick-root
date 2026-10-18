import { listen } from "@tauri-apps/api/event";
import { type ReactElement, useEffect, useState } from "react";
import { FirstRun } from "../firstrun/FirstRun";
import { About } from "./About";
import * as api from "./api";
import { Features } from "./Features";
import { General } from "./General";
import { Nav, PAGES, type PageId } from "./Nav";
import { Providers } from "./Providers";
import { errorText, type SettingsView } from "./types";
import "./settings.css";

function Placeholder({ id }: { id: PageId }) {
  const label = PAGES.find((p) => p.id === id)?.label;
  return (
    <section>
      <h1 className="s-title">{label}</h1>
      <p className="s-help">Coming soon.</p>
    </section>
  );
}

export type View = "first-run" | PageId;

/** Maps a section string (hash or navigate event) to a view; null keeps the current one. */
export function resolveView(section: string | null | undefined, current: View | null): View | null {
  const s = section?.replace(/^#/, "");
  if (s === "first-run") return "first-run";
  if (s === "ai-providers") return "providers";
  return PAGES.find((p) => p.id === s)?.id ?? current;
}

/** Root of the settings window: first run or the pages, driven by the hash and navigate events. */
export function SettingsRoot() {
  const [view, setView] = useState<View>(() => resolveView(location.hash, null) ?? "general");
  useEffect(() => {
    const un = listen<{ section: string | null }>("settings://navigate", (e) =>
      // A bare navigate (tray) while on first run opens the pages.
      setView((v) => resolveView(e.payload.section, v === "first-run" ? "general" : v) ?? v),
    );
    return () => {
      un.then((f) => f());
    };
  }, []);
  return view === "first-run" ? <FirstRun /> : <SettingsApp page={view} onPage={setView} />;
}

export function SettingsApp({ page, onPage }: { page: PageId; onPage: (id: PageId) => void }) {
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [counts, setCounts] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch((e: unknown) => setLoadError(errorText(e)));
    const refreshCounts = () =>
      api
        .listFeatures()
        .then((l) => setCounts(`${l.onCount}/${l.features.length} on`))
        .catch((e: unknown) => console.warn("list_features failed", e));
    refreshCounts();
    const un = api.onFeaturesChanged(refreshCounts);
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (!settings || settings.theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", settings.theme);
  }, [settings]);

  // One slot per page: later cards replace one entry each.
  const pages: Record<PageId, ReactElement> = {
    general: settings ? (
      <General settings={settings} onChange={setSettings} />
    ) : (
      <p className="s-help" role={loadError ? "alert" : undefined}>
        {loadError ?? "Loading…"}
      </p>
    ),
    features: <Features />,
    providers: <Providers />,
    engine: <Placeholder id="engine" />,
    integrations: <Placeholder id="integrations" />,
    about: <About />,
  };

  return (
    <div className="s-shell">
      <Nav active={page} onSelect={onPage} featuresCount={counts} />
      <main className="s-main">{pages[page]}</main>
    </div>
  );
}
