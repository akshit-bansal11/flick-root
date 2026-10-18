import type { ReactNode } from "react";
import "./markdown.css";
import { type Block, type Inline, type ListBlock, type ListItem, parseMarkdown } from "./mdast";

type Open = (url: string) => void;

function host(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function inlineNode(n: Inline, k: number, open: Open): ReactNode {
  switch (n.t) {
    case "text":
      return n.v;
    case "strong":
      return <strong key={k}>{inline(n.c, open)}</strong>;
    case "em":
      return <em key={k}>{inline(n.c, open)}</em>;
    case "code":
      return (
        <code key={k} className="md-code">
          {n.v}
        </code>
      );
    case "br":
      return <br key={k} />;
    case "image":
      return (
        <span key={k} className="md-link">
          {n.alt}
        </span>
      );
    case "link":
      return (
        <button
          key={k}
          type="button"
          className="md-link"
          title={host(n.url)}
          onClick={() => open(n.url)}
        >
          {inline(n.c, open)}
        </button>
      );
  }
}

function inline(nodes: Inline[], open: Open): ReactNode[] {
  return nodes.map((n, k) => inlineNode(n, k, open));
}

function item(it: ListItem, k: number, open: Open): ReactNode {
  return (
    <li key={k}>
      {inline(it.c, open)}
      {it.sub ? list(it.sub, 0, open) : null}
    </li>
  );
}

function list(b: ListBlock, k: number, open: Open): ReactNode {
  const Tag = b.ordered ? "ol" : "ul";
  return <Tag key={k}>{b.items.map((it, j) => item(it, j, open))}</Tag>;
}

function blockNode(b: Block, k: number, open: Open): ReactNode {
  switch (b.t) {
    case "p":
      return <p key={k}>{inline(b.c, open)}</p>;
    case "h": {
      const H = `h${b.level}` as const;
      return <H key={k}>{inline(b.c, open)}</H>;
    }
    case "code":
      return (
        <pre key={k} className="md-pre">
          {b.lang ? <span className="md-lang">{b.lang}</span> : null}
          <code>{b.text}</code>
        </pre>
      );
    case "list":
      return list(b, k, open);
    case "quote":
      return <blockquote key={k}>{blocks(b.c, open)}</blockquote>;
    case "hr":
      return <hr key={k} />;
  }
}

function blocks(bs: Block[], open: Open): ReactNode[] {
  return bs.map((b, k) => blockNode(b, k, open));
}

/** Renders model output as React elements only; links open via `onOpenLink` on click. */
export function Markdown({ text, onOpenLink }: { text: string; onOpenLink: Open }) {
  return <div className="md">{blocks(parseMarkdown(text), onOpenLink)}</div>;
}
