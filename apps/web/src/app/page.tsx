import { APP_DISPLAY_NAME, REPO_URL } from "@/brand";
import { Download } from "@/components/Download";
import { LauncherDemo } from "@/components/LauncherDemo";

const FEATURES = [
  ["Command", "Fuzzy search of installed apps, including Store apps."],
  ["Find", "Fuzzy search of files and folders under the roots you index."],
  ["Query", "Streams an answer from the provider you choose, with fallback."],
  ["Frecency", "Ranks what you use often and recently higher."],
  ["Math, units, currency", "Type 2.5 kg to lb or 250 usd in inr; Enter copies it."],
  ["System commands", "Lock, Sleep, Restart, Shut down, Wi-Fi and more."],
  ["Web shortcuts", "A keyword plus a query opens a URL: g, yt, gh, maps, wiki."],
  ["Project launcher", "Open a project in your editor or terminal by name."],
] as const;

export default function Home() {
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-16 px-6 py-16">
      <section className="flex flex-col items-center gap-6 text-center">
        <h1 className="text-5xl font-semibold">{APP_DISPLAY_NAME}</h1>
        <p className="max-w-xl text-lg text-muted">
          A free, open-source launcher for Windows. No bundled models, keys, account or telemetry;
          nothing AI-related on by default.
        </p>
        <div className="w-full">
          <LauncherDemo />
        </div>
        <Download />
      </section>

      <section aria-labelledby="features">
        <h2 id="features" className="mb-6 text-2xl font-semibold">
          Features
        </h2>
        <ul className="grid gap-4 sm:grid-cols-2">
          {FEATURES.map(([t, d]) => (
            <li key={t} className="rounded-[var(--l-r)] border border-line bg-surface p-4">
              <h3 className="font-semibold">{t}</h3>
              <p className="text-muted">{d}</p>
            </li>
          ))}
        </ul>
      </section>

      <section aria-labelledby="privacy">
        <h2 id="privacy" className="mb-4 text-2xl font-semibold">
          Privacy
        </h2>
        <p className="text-muted">
          {APP_DISPLAY_NAME} is a wrapper. It ships no models, no API keys, no account and no
          default provider. You choose every provider, and Command and Find work fully with no AI at
          all. There is no telemetry and no sign-in.
        </p>
      </section>

      <footer className="border-t border-line pt-6 text-muted">
        MIT licensed ·{" "}
        <a className="underline" href={REPO_URL}>
          GitHub
        </a>
      </footer>
    </main>
  );
}
