import type { Metadata } from "next";
import { APP_DISPLAY_NAME, REPO_URL } from "@/brand";
import { readRepoFile } from "@/lib/content";
import { renderMarkdown } from "@/lib/markdown";
import styles from "../docs/prose.module.css";

export const metadata: Metadata = { title: `Changelog | ${APP_DISPLAY_NAME}` };

export default function Changelog() {
  const src = readRepoFile("CHANGELOG.md");
  if (!src) {
    return (
      <main className={styles.prose}>
        <h1>Changelog</h1>
        <p>
          No releases yet. See <a href={`${REPO_URL}/releases`}>GitHub releases</a>.
        </p>
      </main>
    );
  }
  return <main className={styles.prose}>{renderMarkdown(src)}</main>;
}
