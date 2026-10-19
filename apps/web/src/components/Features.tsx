import Link from "next/link";

type Feature = { title: string; line: string; example: string; since: string; doc?: string };

const FEATURES: Feature[] = [
  {
    title: "Frecency ranking",
    line: "Items you use often and recently rank higher; you can turn it off.",
    example: "chr",
    since: "v0.0.1",
  },
  {
    title: "Math and units",
    line: "Arithmetic, units, currency and time zones; Enter copies the result.",
    example: "2.5 kg to lb",
    since: "v0.0.1",
  },
  {
    title: "System commands",
    line: "Lock, Sleep, Restart, Shut down, Wi-Fi and more; risky ones ask twice.",
    example: "lock",
    since: "v0.0.1",
  },
  {
    title: "Web shortcuts",
    line: "A keyword plus a query opens a URL; edit the list in Settings.",
    example: "yt lofi",
    since: "v0.0.1",
  },
  {
    title: "Project launcher",
    line: "Open a project in your editor or terminal by name.",
    example: "code flick",
    since: "v0.0.1",
  },
  {
    title: "Natural-language finder",
    line: "Describe a file; parsed offline, opened only when the match is clear.",
    example: "pdf from yesterday",
    since: "v0.1.0",
    doc: "natural-language-finder",
  },
  {
    title: "Ports and processes",
    line: "See what holds a port, search processes, end one with confirmation.",
    example: "port 3000",
    since: "v0.1.0",
    doc: "ports-and-processes",
  },
  {
    title: "Media controls",
    line: "Control any player; Spotify and YouTube search work without an account.",
    example: "play lofi on spotify",
    since: "v0.1.0",
    doc: "media-controls",
  },
  {
    title: "Explain this",
    line: "A second hotkey captures selected text and shows it before anything is sent.",
    example: "Alt+Shift+Space",
    since: "v0.1.0",
    doc: "explain-this",
  },
  {
    title: "Quick capture",
    line: "Jot a line to Obsidian, a Markdown file, Notion or a webhook.",
    example: "log 250 lunch #food",
    since: "v0.1.0",
    doc: "quick-capture",
  },
];

const MODES = [
  {
    name: "Command",
    key: ">",
    line: "Fuzzy search of installed apps, including Store, desktop and per-user apps.",
  },
  { name: "Find", key: "/", line: "Fuzzy search of files and folders on every fixed drive." },
  { name: "Query", key: "?", line: "Streams an answer from the provider you choose." },
];

export function Features() {
  return (
    <section aria-labelledby="features">
      <h2 className="mb-2 text-2xl font-semibold">Three modes, one bar</h2>
      <p className="mb-4 text-muted">
        Type the prefix first in an empty bar, or press Tab to cycle.
      </p>
      <ul className="mb-12 grid gap-4 sm:grid-cols-3">
        {MODES.map((m) => (
          <li key={m.name} className="rounded-[var(--l-r)] border border-line bg-surface p-4">
            <h3 className="font-semibold">
              {m.name} <code className="font-mono text-accent">{m.key}</code>
            </h3>
            <p className="text-muted">{m.line}</p>
          </li>
        ))}
      </ul>
      <h2 id="features" className="mb-2 text-2xl font-semibold">
        Features
      </h2>
      <p className="mb-6 text-muted">
        Everything is free and local-first. Anything that needs an account or a model is off by
        default.
      </p>
      <ul className="grid gap-4 sm:grid-cols-2">
        {FEATURES.map((f) => (
          <li key={f.title}>
            <article className="flex h-full flex-col gap-2 rounded-[var(--l-r)] border border-line bg-surface p-4">
              <header className="flex items-baseline justify-between gap-2">
                <h3 className="font-semibold">{f.title}</h3>
                <span className="font-mono text-xs text-muted">{f.since}</span>
              </header>
              <p className="text-muted">{f.line}</p>
              <code className="mt-auto w-fit max-w-full break-words rounded-[var(--l-r2)] border border-line bg-bg px-2 py-1 font-mono text-fg">
                {f.example}
              </code>
              {f.doc && (
                <Link className="w-fit text-accent underline" href={`/docs/feature-${f.doc}`}>
                  Read the docs
                </Link>
              )}
            </article>
          </li>
        ))}
      </ul>
    </section>
  );
}
