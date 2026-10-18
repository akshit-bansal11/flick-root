import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { APP_DISPLAY_NAME } from "@/brand";
import { DOCS, getDoc, readRepoFile } from "@/lib/content";
import { renderMarkdown } from "@/lib/markdown";
import styles from "../prose.module.css";

type Props = { params: Promise<{ slug: string }> };

export const generateStaticParams = () => DOCS.map(({ slug }) => ({ slug }));

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const doc = getDoc((await params).slug);
  return { title: doc ? `${doc.title} | ${APP_DISPLAY_NAME} docs` : "Not found" };
}

export default async function DocPage({ params }: Props) {
  const doc = getDoc((await params).slug);
  const src = doc && readRepoFile(doc.file);
  if (!src) notFound();
  return <main className={styles.prose}>{renderMarkdown(src)}</main>;
}
