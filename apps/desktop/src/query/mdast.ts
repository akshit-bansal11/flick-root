/** Safe Markdown subset parser: text to a typed AST. Raw HTML is never interpreted (ADR-011 rule 4). */

export type Inline =
  | { t: "text"; v: string }
  | { t: "strong"; c: Inline[] }
  | { t: "em"; c: Inline[] }
  | { t: "code"; v: string }
  | { t: "link"; url: string; c: Inline[] }
  | { t: "image"; alt: string }
  | { t: "br" };

export type ListBlock = { t: "list"; ordered: boolean; items: ListItem[] };

export interface ListItem {
  c: Inline[];
  sub?: ListBlock;
}

export type Block =
  | { t: "p"; c: Inline[] }
  | { t: "h"; level: 1 | 2 | 3; c: Inline[] }
  | { t: "code"; lang: string; text: string }
  | ListBlock
  | { t: "quote"; c: Block[] }
  | { t: "hr" };

export const MAX_INPUT = 200_000;
const LOOKAHEAD = 200; // bounds every closer search so parsing stays linear
const MAX_DEPTH = 3;

/** True only for http(s) URLs; everything else (javascript:, data:, file:, relative) is rejected. */
export function isSafeUrl(url: string): boolean {
  try {
    const p = new URL(url).protocol;
    return p === "http:" || p === "https:";
  } catch {
    return false;
  }
}

function find(s: string, needle: string, from: number): number {
  const i = s.slice(from, from + LOOKAHEAD).indexOf(needle);
  return i < 0 ? -1 : from + i;
}

function pushText(out: Inline[], v: string) {
  const last = out[out.length - 1];
  if (last?.t === "text") last.v += v;
  else out.push({ t: "text", v });
}

/** Parse inline spans. Unterminated markers fall back to literal text. */
export function parseInline(s: string, depth = 0): Inline[] {
  const out: Inline[] = [];
  let i = 0;
  while (i < s.length) {
    const ch = s.charAt(i);
    if (ch === "`") {
      const e = find(s, "`", i + 1);
      if (e > i + 1) {
        out.push({ t: "code", v: s.slice(i + 1, e) });
        i = e + 1;
        continue;
      }
    } else if ((ch === "*" || ch === "_") && depth < MAX_DEPTH) {
      const dbl = s.charAt(i + 1) === ch;
      const mark = dbl ? ch + ch : ch;
      const e = find(s, mark, i + mark.length);
      if (e > i + mark.length) {
        const c = parseInline(s.slice(i + mark.length, e), depth + 1);
        out.push({ t: dbl ? "strong" : "em", c });
        i = e + mark.length;
        continue;
      }
    } else if (ch === "[" || (ch === "!" && s.charAt(i + 1) === "[")) {
      const img = ch === "!";
      const open = img ? i + 1 : i;
      const close = find(s, "]", open + 1);
      if (close > 0 && s.charAt(close + 1) === "(") {
        const end = find(s, ")", close + 2);
        if (end > 0) {
          const label = s.slice(open + 1, close);
          const url = s.slice(close + 2, end).trim();
          const c =
            depth < MAX_DEPTH ? parseInline(label, depth + 1) : [{ t: "text" as const, v: label }];
          if (img) out.push({ t: "image", alt: label });
          else if (isSafeUrl(url)) out.push({ t: "link", url, c });
          else out.push(...c);
          i = end + 1;
          continue;
        }
      }
    } else if (ch === "h" && (s.startsWith("http://", i) || s.startsWith("https://", i))) {
      const m = /^https?:\/\/[^\s<>]+/.exec(s.slice(i, i + 2000));
      const url = m?.[0].replace(/[.,;:!?)\]'"]+$/, "") ?? "";
      if (isSafeUrl(url)) {
        out.push({ t: "link", url, c: [{ t: "text", v: url }] });
        i += url.length;
        continue;
      }
    }
    pushText(out, ch);
    i++;
  }
  return out;
}

const FENCE = /^ {0,3}```(\S*)/;
const HR = /^ {0,3}([-*_])(?: *\1){2,} *$/;
const HEADING = /^ {0,3}(#{1,3}) +(.*)$/;
const QUOTE = /^ {0,3}>/;
const ITEM = /^( *)([-*+]|\d{1,9}[.)]) +(.*)$/;

function startsBlock(l: string): boolean {
  return FENCE.test(l) || HR.test(l) || HEADING.test(l) || QUOTE.test(l) || ITEM.test(l);
}

function parseList(lines: string[], start: number): { block: ListBlock; next: number } {
  const root: ListBlock = { t: "list", ordered: /^ *\d/.test(lines[start] ?? ""), items: [] };
  let sub: ListBlock | undefined;
  let i = start;
  while (i < lines.length) {
    const line = lines[i] ?? "";
    const m = ITEM.exec(line);
    if (!m || HR.test(line)) break;
    const c = parseInline((m[3] ?? "").trim());
    const parent = root.items[root.items.length - 1];
    if ((m[1]?.length ?? 0) >= 2 && parent) {
      sub ??= { t: "list", ordered: /^\d/.test(m[2] ?? ""), items: [] };
      sub.items.push({ c });
      parent.sub = sub;
    } else {
      sub = undefined;
      root.items.push({ c });
    }
    i++;
  }
  return { block: root, next: i };
}

function parseLines(lines: string[], depth: number): Block[] {
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? "";
    const fence = FENCE.exec(line);
    const heading = HEADING.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^ *``` *$/.test(lines[i] ?? "")) body.push(lines[i++] ?? "");
      i++; // closing fence, or past the end for an open fence
      blocks.push({ t: "code", lang: fence[1] ?? "", text: body.join("\n") });
    } else if (!line.trim()) {
      i++;
    } else if (HR.test(line)) {
      blocks.push({ t: "hr" });
      i++;
    } else if (heading) {
      const level = Math.min(heading[1]?.length ?? 1, 3) as 1 | 2 | 3;
      blocks.push({ t: "h", level, c: parseInline((heading[2] ?? "").trim()) });
      i++;
    } else if (QUOTE.test(line)) {
      const inner: string[] = [];
      while (i < lines.length && QUOTE.test(lines[i] ?? "")) {
        inner.push((lines[i++] ?? "").replace(/^ {0,3}> ?/, ""));
      }
      blocks.push({
        t: "quote",
        c:
          depth < MAX_DEPTH
            ? parseLines(inner, depth + 1)
            : [{ t: "p", c: parseInline(inner.join(" ")) }],
      });
    } else if (ITEM.test(line)) {
      const { block, next } = parseList(lines, i);
      blocks.push(block);
      i = next;
    } else {
      const c: Inline[] = [];
      let first = true;
      while (
        i < lines.length &&
        (lines[i] ?? "").trim() &&
        (first || !startsBlock(lines[i] ?? ""))
      ) {
        if (!first) c.push({ t: "br" });
        c.push(...parseInline((lines[i++] ?? "").trim()));
        first = false;
      }
      blocks.push({ t: "p", c });
    }
  }
  return blocks;
}

/** Parse Markdown (possibly a streaming prefix) into blocks. Never throws; input capped at 200 KB. */
export function parseMarkdown(text: string): Block[] {
  const src = text.length > MAX_INPUT ? `${text.slice(0, MAX_INPUT)}\n\n… (truncated)` : text;
  return parseLines(src.split(/\r?\n/), 0);
}
