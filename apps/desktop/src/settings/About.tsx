import { useEffect, useState } from "react";
import { APP_DISPLAY_NAME } from "../brand";
import * as api from "./api";
import { SettingRow } from "./Row";
import type { About as AboutInfo } from "./types";
import "./features.css";

/** Version, license and links; links open through Rust, which validates http/https. */
export function About() {
  const [about, setAbout] = useState<AboutInfo | null>(null);
  useEffect(() => {
    api.getAbout().then(setAbout, () => setAbout(null));
  }, []);
  if (!about) return null;
  const links: [string, string, string][] = [
    ["Source code", about.links.repo, "The source code of every release."],
    ["Docs", about.links.docs, "Setup guides and reference."],
    ["Changelog", about.links.changelog, "What changed in each release."],
    ["Theme credits", about.links.themes, "Authors and licences of the bundled theme ports."],
  ];
  return (
    <section aria-label="About">
      <h1 className="s-title">About</h1>
      <p className="s-desc s-prose">{`${APP_DISPLAY_NAME} is free and open source.`}</p>
      <h2 className="sec"># about</h2>
      <SettingRow
        label="Version"
        value={about.version}
        preview={{ title: "# version", body: `The installed ${APP_DISPLAY_NAME} version.` }}
      />
      <SettingRow
        label="Licence"
        value={about.license}
        preview={{
          title: "# licence",
          body: `${APP_DISPLAY_NAME} is released under this licence.`,
        }}
      />
      <h2 className="sec"># links</h2>
      <p className="s-desc s-prose">Some themes are ports of editor themes; see the credits.</p>
      {links.map(([label, url, body]) => (
        <SettingRow
          key={label}
          label={label}
          hint={url}
          preview={{ title: `# ${label.toLowerCase()}`, body }}
        >
          <button
            type="button"
            className="btn"
            aria-label={`Open ${label.toLowerCase()}`}
            onClick={() => api.openLink(url)}
          >
            open
          </button>
        </SettingRow>
      ))}
    </section>
  );
}
