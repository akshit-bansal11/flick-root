import { listen } from "@tauri-apps/api/event";
import { type ReactElement, useEffect, useState } from "react";
import { FirstRun } from "../firstrun/FirstRun";
import * as ipc from "../ipc";
import { fromEvent, idFor } from "../shortcuts";
import { applyAppearance } from "../theme";
import { Tour } from "../tour/Tour";
import { About } from "./About";
import { Appearance } from "./Appearance";
import * as api from "./api";
import { Decision } from "./Decision";
import { General } from "./General";
import { Integrations } from "./Integrations";
import { Nav, PAGES, type PageId } from "./Nav";
import { Prefixes } from "./Prefixes";
import { Providers } from "./Providers";
import { errorText, type SettingsView } from "./types";
import "./settings.css";

export type View = "first-run" | PageId;

/** Maps a section string (hash or navigate event) to a view; null keeps the current one. */
export function resolveView(section: string | null | undefined, current: View | null): View | null {
  const s = section?.replace(/^#/, "");
  if (s === "first-run") return "first-run";
  if (s === "ai-providers") return "providers";
  if (s === "features") return "prefixes";
  return PAGES.find((p) => p.id === s)?.id ?? current;
}

const TEXT_FIELD = "input, textarea, select, [contenteditable='true']";

/** Next page id, wrapping; the page list order is the Nav order. */
export function stepPage(page: PageId, delta: number): PageId {
  const i = PAGES.findIndex((p) => p.id === page);
  return PAGES[(i + delta + PAGES.length) % PAGES.length]?.id ?? page;
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
  const [touring, setTouring] = useState(false);

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

  // Window shortcuts. Page keys never fire while typing; Esc and Ctrl+W always close, except Esc inside a dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // The tour owns Esc and its own keys while it is open.
      // A field that handled the key (hotkey capture) owns it.
      if (touring || e.defaultPrevented) return;
      const id = idFor("settings", fromEvent(e));
      if (!id) return;
      const target = e.target instanceof Element ? e.target : null;
      if (id === "closeSettings" || id === "closeSettingsW") {
        if (id === "closeSettings" && target?.closest("dialog, [role='dialog']")) return;
        e.preventDefault();
        ipc.closeWindow().catch((err: unknown) => console.warn("close failed", err));
        return;
      }
      if (target?.closest(TEXT_FIELD)) return;
      e.preventDefault();
      onPage(stepPage(page, id === "pageNext" ? 1 : -1));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [page, onPage, touring]);

  // Apply the saved appearance on load and whenever Rust says it changed.
  useEffect(() => {
    const apply = () =>
      api
        .getAppearance()
        .then((a) => applyAppearance(document.documentElement, { ...a, glassAvailable: true }))
        .catch((e: unknown) => console.warn("get_appearance failed", e));
    apply();
    const un = api.onSettingsChanged((p) => {
      if (p.section === "appearance") apply();
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // One slot per page: later cards replace one entry each.
  const pages: Record<PageId, ReactElement> = {
    general: settings ? (
      <General settings={settings} onChange={setSettings} onTour={() => setTouring(true)} />
    ) : (
      <p className="s-help" role={loadError ? "alert" : undefined}>
        {loadError ?? "Loading…"}
      </p>
    ),
    appearance: <Appearance />,
    prefixes: <Prefixes />,
    providers: <Providers />,
    engine: <Decision />,
    integrations: <Integrations />,
    about: <About />,
  };

  if (touring) return <Tour onDone={() => setTouring(false)} />;

  return (
    <div className="s-shell">
      <Nav active={page} onSelect={onPage} featuresCount={counts} />
      <main key={page} className="s-main">
        {pages[page]}
      </main>
    </div>
  );
}
