import type { Metadata } from "next";
import type { ReactNode } from "react";
import { APP_DISPLAY_NAME } from "@/brand";
import { SiteNav } from "@/components/SiteNav";
import "./globals.css";

const description =
  "A free, open-source, keyboard-first launcher for Windows. Command, Find and Query modes in one bar. No bundled models, keys, account or telemetry.";

export const metadata: Metadata = {
  title: `${APP_DISPLAY_NAME}: a keyboard-first launcher for Windows`,
  description,
  openGraph: { title: APP_DISPLAY_NAME, description, type: "website" },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className="h-full antialiased">
      <body className="min-h-full flex flex-col">
        <SiteNav />
        {children}
      </body>
    </html>
  );
}
