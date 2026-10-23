import { REPO_URL } from "@/brand";

export function Download() {
  return (
    <div className="flex flex-col items-center gap-2">
      <a
        href={`${REPO_URL}/releases/latest`}
        className="rounded-[var(--l-r-control)] bg-accent px-6 py-3 font-semibold text-ink"
      >
        Download for Windows
      </a>
      <small className="text-muted">Releases appear here once published</small>
    </div>
  );
}
