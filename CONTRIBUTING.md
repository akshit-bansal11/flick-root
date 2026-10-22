# Contributing to Flick

Thanks for looking at Flick. It is a free, open-source, keyboard-first launcher for Windows. It is a wrapper: it ships no models, no keys and no default provider, and it has no telemetry. Changes that keep it small, fast and private are the easiest to merge.

## Prerequisites

- Windows with the Microsoft Edge WebView2 runtime.
- Rust stable with the `x86_64-pc-windows-msvc` target and the MSVC Build Tools.
- Node.js (CI uses Node 24) and pnpm. The workspace pins pnpm 10.28.1.

## Setup and commands

Install dependencies once from the repository root:

```
pnpm install
```

Run the desktop app in development:

```
pnpm --filter desktop tauri dev
```

Rust checks, one crate at a time or the whole workspace:

```
cargo check -p <crate>
cargo test -p <crate>
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Benchmarks (criterion):

```
cargo bench -p launcher-core
cargo bench -p index-files
```

UI checks:

```
pnpm --filter desktop typecheck
pnpm --filter desktop test
pnpm lint
```

Landing site:

```
pnpm --filter web dev
pnpm --filter web build
```

CI runs the same gates on every pull request, split into jobs that run only when their files changed: `rust` (format, clippy, tests, cargo deny, cargo audit) for `crates/`, `apps/desktop/src-tauri/` and the Cargo files; `js` (typecheck, lint, JS tests, pnpm audit) for the desktop UI and any JS, TS, JSON or CSS file; `web` (the web build) for `apps/web/`. A change to `ci.yml` itself, or a manual run (`gh workflow run ci.yml`), runs every job.

## Code rules

Rust:

- Libraries use `thiserror` error enums. `anyhow` is only for `apps/desktop/src-tauri`.
- No `unwrap()` or `expect()` outside tests. Errors carry context.
- `unsafe` is allowed only in `crates/index-apps` and `crates/win`. Each block needs a `// SAFETY:` comment that states the invariant.
- No blocking calls inside async code. Use `spawn_blocking` for COM, disk scans and SQLite.
- Public items have doc comments. Clippy must be clean with `-D warnings`.
- The brand name lives only in `crates/launcher-core/src/brand.rs`.

TypeScript and UI:

- Strict mode, no `any`, Biome formatting and lint clean, `tsc --noEmit` clean.
- One file, one purpose. Use feature folders (`launcher/`, `settings/`, `query/`), not folders by type.
- Colors, radius, row height and fonts come only from the tokens in `tokens.css`. No literal colors in components.
- No business logic in components. Keyboard state is a pure reducer with tests.
- The brand name lives only in `src/brand.ts`.

Privacy and secrets:

- Provider keys follow ADR-011: Windows Credential Manager only, write-only from the UI, never returned by any command, never in config, SQLite, fixtures or snapshots, and redacted from logs and panics.
- The WebView makes no network calls. Rust does. Keep the CSP and Tauri capabilities minimal.
- Model output is untrusted and can never trigger an action.
- No telemetry and no network calls on the keystroke path.

## Adding a feature

Every feature is a module behind the `Feature` trait in `crates/features` (ADR-009). The trait covers `id()`, `title()`, `default_enabled()`, `settings_schema()`, `start(ctx)`, `stop()`, `query(&Input)` and `execute(&Item, Action)`.

- `query()` is synchronous and must stay under 0.5 ms. Do slow work in `start()` and keep the result. Add a criterion bench for it.
- Each feature has a toggle at `[features.<id>] enabled`. Toggles apply instantly, with no restart.
- Off means stopped. `stop()` must drop everything the feature holds: watchers, timers, caches and network clients. Test that stopping releases them.
- Spawn background work only through `ctx.tasks`.
- The options page is generated from `settings_schema()`. Do not write a hand-made settings page for a feature.
- Keep shared state inside the feature struct, not in a module-level static.

## Performance budgets

These are acceptance criteria. A change that breaks a budget is a bug.

- Hotkey to visible: p95 at most 50 ms.
- Command match for 2,000 apps: at most 2 ms per query.
- Each feature's `query()`: at most 0.5 ms.
- Find match over 300,000 paths: p95 at most 25 ms.
- Launch call returned: at most 30 ms. Spawn the app and never wait on it.
- Idle RAM: at most 120 MB including WebView2. With every feature off, nothing polls.

Measured numbers are recorded in `docs/performance.md`.

## Commit messages

Commits use a prefix that maps to a changelog group in `cliff.toml`:

| Prefix | Changelog group | Use for |
| --- | --- | --- |
| `feat/` | Features | A new user-visible feature |
| `fix/` | Fixes | A bug fix |
| `up/` | Updates | A change to an existing feature |
| `add-p/` | Packages | A dependency added |
| `add/` | Added | Something new that is not a feature |
| `rem/` | Removed | Something removed |
| `sm/` | Small changes | Small refactors or polish |
| `init/` | Init | Project setup |
| `hide/` | (not in changelog) | Commits that should not appear in the changelog |

Example: `fix/ search: keep Store apps when the index is rebuilt`.

`CHANGELOG.md` is generated by git-cliff from these prefixes. Do not edit it by hand.

## Pull requests

Before you open a pull request, check that:

- CI is green.
- New logic has tests. Rust tests go next to the crate, UI logic has tests in the same folder.
- Your change stays within the files it needs. Unrelated cleanup goes in its own commit.
- New features have a toggle, a stop path and a bench for `query()`.
- No secret, key or personal data appears in code, fixtures, snapshots or logs.
- Docs (README, ADRs) match the code you changed.

Report security issues privately. See [SECURITY.md](SECURITY.md).
