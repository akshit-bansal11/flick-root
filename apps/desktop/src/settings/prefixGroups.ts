import type { MODES } from "../launcher/reducer";
import type { ModeToggle } from "./types";

/** A section of the Prefixes page. `mode` names the prefix (glyph, label, description come from `MODES`). */
export interface PrefixGroup {
  id: string;
  /** The launcher prefix this group belongs to; null for groups that have none. */
  mode: keyof typeof MODES | null;
  /** Heading and one-line description for groups without a prefix. */
  title?: string;
  desc?: string;
  /** A mode switch shown first in the group. */
  toggle?: { key: ModeToggle; label: string };
  /** Feature ids, in display order. */
  features: readonly string[];
}

/** Prefix order follows `MODES`; the last group holds features that no prefix searches. */
export const PREFIX_GROUPS: readonly PrefixGroup[] = [
  {
    id: "find",
    mode: "find",
    toggle: { key: "find", label: "Files and folders" },
    features: ["nlfind"],
  },
  {
    id: "commands",
    mode: "commands",
    features: ["syscmd", "web", "media", "ports", "capture", "links"],
  },
  { id: "calc", mode: "calc", features: ["math"] },
  { id: "query", mode: "query", toggle: { key: "query", label: "Ask" }, features: ["explain"] },
  { id: "projects", mode: "projects", features: ["projects"] },
  {
    id: "everywhere",
    mode: null,
    title: "Everywhere",
    desc: "applies to every search",
    features: ["frecency"],
  },
];

/** The group for features that are in none of `PREFIX_GROUPS`, so a new feature never disappears. */
export const OTHER_GROUP: PrefixGroup = {
  id: "other",
  mode: null,
  title: "Other",
  desc: "modules without a prefix",
  features: [],
};
