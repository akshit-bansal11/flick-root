import { APP_DISPLAY_NAME, REPO_URL } from "@/brand";
import { Download } from "@/components/Download";
import { Features } from "@/components/Features";
import { LauncherDemo } from "@/components/LauncherDemo";

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

      <Features />

      <section aria-labelledby="privacy">
        <h2 id="privacy" className="mb-4 text-2xl font-semibold">
          Privacy
        </h2>
        <p className="text-muted">
          {APP_DISPLAY_NAME} is a wrapper. It ships no models, no API keys, no account and no
          default provider. You choose every provider, and Open, Commands, Calc and Projects work
          fully with no AI at all. There is no telemetry and no sign-in.
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
