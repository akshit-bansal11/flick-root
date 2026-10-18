import { lexer, type Token, type Tokens } from "marked";
import { createElement as h, type ReactNode } from "react";
import { DOCS } from "./content";

const byFile = new Map(DOCS.map((d) => [d.file.split("/").pop() ?? "", d.slug]));

/** Rewrites relative .md links between published docs; blanks script-like schemes. */
export function resolveHref(href: string): string {
  if (/^\s*(javascript|data|vbscript):/i.test(href)) return "#";
  const m = /^(?:\.{1,2}\/)*(?:docs\/)?(?:adr\/)?([^/#?]+\.md)(#.*)?$/.exec(href);
  const slug = m && byFile.get(m[1] ?? "");
  return m && slug ? `/docs/${slug}${m[2] ?? ""}` : href;
}

function inline(tokens: Token[] | undefined): ReactNode[] {
  return (tokens ?? []).map((t, i) => node(t, i));
}

function node(t: Token, key: number): ReactNode {
  switch (t.type) {
    case "heading": {
      const x = t as Tokens.Heading;
      return h(`h${Math.min(Math.max(x.depth, 1), 6)}`, { key }, inline(x.tokens));
    }
    case "paragraph":
      return h("p", { key }, inline((t as Tokens.Paragraph).tokens));
    case "text": {
      const x = t as Tokens.Text;
      return x.tokens ? h("span", { key }, inline(x.tokens)) : x.text;
    }
    case "escape":
    case "html":
      return (t as Tokens.Escape).text;
    case "strong":
      return h("strong", { key }, inline((t as Tokens.Strong).tokens));
    case "em":
      return h("em", { key }, inline((t as Tokens.Em).tokens));
    case "del":
      return h("del", { key }, inline((t as Tokens.Del).tokens));
    case "codespan":
      return h("code", { key }, (t as Tokens.Codespan).text);
    case "code": {
      const x = t as Tokens.Code;
      return h(
        "pre",
        { key },
        h("code", { className: x.lang ? `language-${x.lang}` : undefined }, x.text),
      );
    }
    case "list": {
      const x = t as Tokens.List;
      const items = x.items.map((it, i) => h("li", { key: i }, inline(it.tokens)));
      return x.ordered
        ? h("ol", { key, start: x.start === "" ? undefined : x.start }, items)
        : h("ul", { key }, items);
    }
    case "table": {
      const x = t as Tokens.Table;
      const cell = (tag: "th" | "td") => (c: Tokens.TableCell, i: number) =>
        h(tag, { key: i, style: c.align ? { textAlign: c.align } : undefined }, inline(c.tokens));
      return h(
        "table",
        { key },
        h("thead", null, h("tr", null, x.header.map(cell("th")))),
        h(
          "tbody",
          null,
          x.rows.map((r, i) => h("tr", { key: i }, r.map(cell("td")))),
        ),
      );
    }
    case "blockquote":
      return h("blockquote", { key }, inline((t as Tokens.Blockquote).tokens));
    case "hr":
      return h("hr", { key });
    case "br":
      return h("br", { key });
    case "link": {
      const x = t as Tokens.Link;
      return h(
        "a",
        { key, href: resolveHref(x.href), title: x.title ?? undefined },
        inline(x.tokens),
      );
    }
    case "image":
      return (t as Tokens.Image).text;
    default:
      return null;
  }
}

/** Renders trusted repo Markdown to React elements; raw HTML becomes plain text. */
export function renderMarkdown(src: string): ReactNode[] {
  return inline(lexer(src));
}
