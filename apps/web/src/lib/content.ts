import { readFileSync } from "node:fs";
import path from "node:path";

export type DocGroup = "Guides" | "Project" | "Decisions";
export type Doc = { slug: string; title: string; group: DocGroup; file: string };

const adr = (n: string, name: string, title: string): Doc => ({
  slug: `adr-${n}-${name}`,
  title: `ADR-${n}: ${title}`,
  group: "Decisions",
  file: `docs/adr/ADR-${n}-${name}.md`,
});

const feature = (name: string, title: string): Doc => ({
  slug: `feature-${name}`,
  title,
  group: "Guides",
  file: `docs/features/${name}.md`,
});

/** Fixed allow-list: slugs map to repo files only through this table. */
export const DOCS: Doc[] = [
  { slug: "readme", title: "README", group: "Guides", file: "README.md" },
  feature("natural-language-finder", "Natural-language finder"),
  feature("ports-and-processes", "Ports and processes"),
  feature("media-controls", "Media controls"),
  feature("explain-this", "Explain this"),
  feature("quick-capture", "Quick capture"),
  feature("programs", "Programs"),
  feature("links-and-paths", "Links and paths"),
  feature("search", "Search"),
  feature("system-commands", "System commands"),
  feature("thinking-modes", "Thinking modes"),
  feature("appearance", "Appearance"),
  feature("tutorial", "Tutorial"),
  { slug: "architecture", title: "Architecture", group: "Guides", file: "docs/architecture.md" },
  { slug: "performance", title: "Performance", group: "Guides", file: "docs/performance.md" },
  { slug: "roadmap", title: "Roadmap", group: "Guides", file: "docs/roadmap.md" },
  { slug: "contributing", title: "Contributing", group: "Project", file: "CONTRIBUTING.md" },
  { slug: "security", title: "Security", group: "Project", file: "SECURITY.md" },
  adr("001", "shell-and-ui", "Shell and UI"),
  adr("002", "app-index", "App index"),
  adr("003", "file-index", "File index"),
  adr("004", "matching", "Matching"),
  adr("005", "natural-language-finder", "Natural-language finder"),
  adr("006", "providers", "Providers"),
  adr("007", "storage-and-secrets", "Storage and secrets"),
  adr("008", "repo-and-web", "Repo and web"),
  adr("009", "feature-modules", "Feature modules"),
  adr("010", "integrations", "Integrations"),
  adr("011", "secret-security", "Secret security"),
  adr("012", "prefixes-and-plain-text", "Prefixes and plain text"),
  adr("013", "tray-menu", "Tray menu"),
  adr("014", "settings-backdrop", "Settings backdrop"),
];

const root = path.resolve(process.cwd(), "../..");

/** Reads a repo file at build time; null when absent. */
export function readRepoFile(file: string): string | null {
  try {
    return readFileSync(path.join(root, file), "utf8");
  } catch {
    return null;
  }
}

export const getDoc = (slug: string) => DOCS.find((d) => d.slug === slug);
