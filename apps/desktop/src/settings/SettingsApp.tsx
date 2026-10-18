import { type ReactElement, useEffect, useState } from "react";
import * as api from "./api";
import { General } from "./General";
import { Nav, PAGES, type PageId } from "./Nav";
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

export function SettingsApp() {
  const [page, setPage] = useState<PageId>("general");
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
    features: <Placeholder id="features" />,
    providers: <Placeholder id="providers" />,
    engine: <Placeholder id="engine" />,
    integrations: <Placeholder id="integrations" />,
    about: <Placeholder id="about" />,
  };

  return (
    <div className="s-shell">
      <Nav active={page} onSelect={setPage} featuresCount={counts} />
      <main className="s-main">{pages[page]}</main>
    </div>
  );
}
