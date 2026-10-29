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
    example: "= 2.5 kg to lb",
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
    title: "Links and paths",
    line: "Type a web address or a folder path and press Enter to open it.",
    example: "github.com/flick",
    since: "v0.0.1",
    doc: "links-and-paths",
  },
  {
    title: "Natural-language finder",
    line: "Describe a file; parsed offline, opened only when the match is clear.",
    example: "pdf from yesterday",
    since: "v0.0.1",
    doc: "natural-language-finder",
  },
  {
    title: "Ports and processes",
    line: "See what holds a port, search processes, end one with confirmation.",
    example: "port 3000",
    since: "v0.0.1",
    doc: "ports-and-processes",
  },
  {
    title: "Media controls",
    line: "Control any player; Spotify and YouTube search work without an account.",
    example: "play lofi on spotify",
    since: "v0.0.1",
    doc: "media-controls",
  },
  {
    title: "Explain this",
    line: "A second hotkey captures selected text and shows it before anything is sent.",
    example: "Alt+Shift+Space",
    since: "v0.0.1",
    doc: "explain-this",
  },
  {
    title: "Quick capture",
    line: "Jot a line to Obsidian, a Markdown file, Notion or a webhook.",
    example: "log 250 lunch #food",
    since: "v0.0.1",
    doc: "quick-capture",
  },
  {
    title: "Programs on all drives",
    line: "/ also lists programs found on every fixed drive, below installed apps.",
    example: "/ setup",
    since: "v0.0.1",
    doc: "programs",
  },
  {
    title: "Thinking modes",
    line: "Pick the provider and model that answers first; Tab switches.",
    example: "? then Tab",
    since: "v0.0.1",
    doc: "thinking-modes",
  },
  {
    title: "Themes",
    line: "A flat, monospace look. Dozens of ported editor themes plus its own defaults, with separate dark and light picks.",
    example: "> theme",
    since: "v0.0.1",
    doc: "appearance",
  },
  {
    title: "Mode tabs",
    line: "A tab strip above the results: all, or one tab per prefix. Ctrl Tab moves to the next mode.",
    example: "ctrl tab",
    since: "v0.0.1",
  },
  {
    title: "Pins and recents",
    line: "Pin the rows you reach for first; recent picks are one keystroke away.",
    example: "ctrl p",
    since: "v0.0.1",
  },
  {
    title: "Tutorial",
    line: "A welcome screen and an interactive tour; replay it in Settings.",
    example: "Settings, General",
    since: "v0.0.1",
    doc: "tutorial",
  },
];

const MODES = [
  { name: "Open", key: "/", line: "Apps, programs, files and folders." },
  {
    name: "Commands",
    key: ">",
    line: "System commands, web shortcuts, media, ports and processes, quick capture, links.",
  },
  { name: "Calc", key: "=", line: "Math, units, currency, time zones and number bases." },
  {
    name: "Ask",
    key: "?",
    line: "Answers from your AI providers, labelled with provider and model.",
  },
];

export function Features() {
  return (
    <section aria-labelledby="features">
      <h2 className="mb-2 text-2xl font-semibold">Five prefixes, one bar</h2>
      <p className="mb-4 text-muted">
        Type a prefix first to narrow the search. Plain text with no prefix searches apps and
        commands and always offers Find, Web and Ask.
      </p>
      <ul className="mb-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        {MODES.map((m) => (
          <li key={m.name} className="rounded-[var(--l-r-card)] border border-line bg-surface p-4">
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
            <article className="flex h-full flex-col gap-2 rounded-[var(--l-r-card)] border border-line bg-surface p-4">
              <header className="flex items-baseline justify-between gap-2">
                <h3 className="font-semibold">{f.title}</h3>
                <span className="font-mono text-xs text-muted">{f.since}</span>
              </header>
              <p className="text-muted">{f.line}</p>
              <code className="mt-auto w-fit max-w-full break-words rounded-[var(--l-r-key)] border border-line bg-bg px-2 py-1 font-mono text-fg">
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
