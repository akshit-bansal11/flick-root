import { useEffect, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import * as api from "./api";
import type { About as AboutInfo } from "./types";
import "./providers.css";

/** Version, license and links; links open through Rust, which validates http/https. */
export function About() {
  const [about, setAbout] = useState<AboutInfo | null>(null);
  useEffect(() => {
    api.getAbout().then(setAbout, () => setAbout(null));
  }, []);
  if (!about) return null;
  const links: [string, string][] = [
    ["Repository", about.links.repo],
    ["Docs", about.links.docs],
    ["Changelog", about.links.changelog],
  ];
  return (
    <section aria-label="About">
      <h1 className="s-title">About</h1>
      <p>{`${APP_DISPLAY_NAME} ${about.version} · ${about.license}`}</p>
      <ul className="pv-links">
        {links.map(([label, url]) => (
          <li key={label}>
            <button type="button" className="pv-link" onClick={() => api.openLink(url)}>
              {label}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
