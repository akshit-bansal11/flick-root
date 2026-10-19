export interface Segment {
  text: string;
  hit: boolean;
  /** Start offset in the title; unique per segment, usable as a React key. */
  at: number;
}

/** Marks the characters of `title` matched by `query`: a contiguous run if one exists, else a greedy subsequence. */
export function highlight(title: string, query: string): Segment[] {
  const q = query.trim().toLowerCase();
  if (!q) return [{ text: title, hit: false, at: 0 }];
  const t = title.toLowerCase();
  // toLowerCase can change length for a few characters; skip highlighting then.
  if (t.length !== title.length) return [{ text: title, hit: false, at: 0 }];
  const marked = new Array<boolean>(title.length).fill(false);
  const at = t.indexOf(q);
  if (at >= 0) {
    marked.fill(true, at, at + q.length);
  } else {
    let from = 0;
    for (const ch of q) {
      const i = t.indexOf(ch, from);
      if (i < 0) return [{ text: title, hit: false, at: 0 }];
      marked[i] = true;
      from = i + 1;
    }
  }
  const out: Segment[] = [];
  for (let i = 0; i < title.length; i++) {
    const hit = marked[i] === true;
    const last = out[out.length - 1];
    if (last && last.hit === hit) last.text += title[i];
    else out.push({ text: title[i] ?? "", hit, at: i });
  }
  return out;
}
