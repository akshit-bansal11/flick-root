# ADR-008: Repo and web

Status: Accepted (2026-10-19)

## Context

Flick has a desktop app, several Rust crates, a TypeScript UI, and a public landing and docs site. The app and the site share build tooling and release history. They need one place to live, with each part built by its own toolchain.

## Decision

- One repository holds a pnpm workspace and a Cargo workspace.
- The landing and docs site is `apps/web`, built with the current stable Next.js (App Router), Tailwind and Framer Motion.
- The site deploys on Vercel. A custom subdomain is added later.

Current state:

- `apps/web` exists as a scaffold. Deployment is pending; no production URL is claimed in this ADR.
- Commands: `pnpm --filter web dev` and `pnpm --filter web build`.
- The dependency list for `apps/web` is not verified in this ADR. Check `apps/web/package.json` before relying on it.

## Alternatives

- Separate repositories for the app and the site. Rejected: releases and docs must match the app's version, and one repo keeps them in step.
- A static site generator without a framework. Not chosen: the decision names Next.js App Router.

## Consequences

- One CI and release process covers the app and the site.
- The site build must not break the desktop build; each package is checked with its own filter command.
- Until deployment is set up, the site has no public URL and no uptime guarantee.
