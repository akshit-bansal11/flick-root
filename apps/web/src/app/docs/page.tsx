import type { Metadata } from "next";
import Link from "next/link";
import { APP_DISPLAY_NAME } from "@/brand";
import { DOCS, type DocGroup } from "@/lib/content";
import styles from "./prose.module.css";

export const metadata: Metadata = { title: `Docs | ${APP_DISPLAY_NAME}` };

const groups: DocGroup[] = ["Guides", "Project", "Decisions"];

export default function DocsIndex() {
  return (
    <main className={styles.prose}>
      <h1>Docs</h1>
      {groups.map((g) => (
        <section key={g}>
          <h2>{g}</h2>
          <ul>
            {DOCS.filter((d) => d.group === g).map((d) => (
              <li key={d.slug}>
                <Link href={`/docs/${d.slug}`}>{d.title}</Link>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </main>
  );
}
