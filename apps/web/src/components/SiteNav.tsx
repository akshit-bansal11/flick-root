import Link from "next/link";
import { APP_DISPLAY_NAME, REPO_URL } from "@/brand";

const LINKS = [
  { href: "/", label: "Home" },
  { href: "/docs", label: "Docs" },
  { href: "/changelog", label: "Changelog" },
];

export function SiteNav() {
  return (
    <nav aria-label="Main" className="flex items-center gap-6 border-b border-line px-6 py-4">
      <strong className="mr-auto">{APP_DISPLAY_NAME}</strong>
      {LINKS.map((l) => (
        <Link key={l.href} href={l.href} className="text-muted hover:text-fg">
          {l.label}
        </Link>
      ))}
      <a href={REPO_URL} className="text-muted hover:text-fg">
        GitHub
      </a>
    </nav>
  );
}
