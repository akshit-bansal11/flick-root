# Flick

Flick is a free, open-source, keyboard-first launcher for Windows. Commands, apps, Find and Ask share one bar, each behind a one-character prefix: run a command, open an app or program, find a file, or ask a model, without leaving the keyboard. It is a wrapper. It ships no models, no API keys, no account and no default provider, and nothing AI-related is switched on out of the box. You choose every provider, and commands, apps and Find work fully with no AI at all. There is no telemetry, no sign-in, and the license is MIT.

## Status

Flick is at v0.2.0. Built so far:

- Apps (`!`): fuzzy search of installed apps, including Store apps, as Start's "All apps" lists them, plus programs found on your drives.
- Commands (`>`): system commands, math, units, web shortcuts, ports, media, capture.
- Find (`/`): fuzzy search of files and folders under the indexed roots, with Open, Open file location and Copy path.
- Ask (`?`): streams an answer from the provider and model you pick, with Fast/Think, thinking modes and provider fallback.
- Features: frecency, inline math and units, system commands, web shortcuts, project launcher.
- Settings window (General, Appearance, Features, AI providers, About), first run and a skippable tutorial.

Added in v0.1.0: Natural-language finder, ports and processes, media controls, Explain this, and Quick capture (see Features below and [docs/features/](docs/features/)).

Added in v0.2.0: prefix-only modes, programs on all drives, thinking modes, a liquid glass launcher with six themes, and the tutorial.

## Install

Download the installer from the GitHub Releases page of [akshit-bansal11/flick](https://github.com/akshit-bansal11/flick/releases) once a release is published. The repository is currently private.

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC". Choose **More info**, then **Run anyway**. Each release publishes SHA-256 checksums so you can verify the download.

## Hotkeys and modes

Modes are chosen only by a prefix typed first in the empty bar; there is no Tab switcher. Plain text with no prefix searches nothing and shows the four prefixes. Backspace on an empty input leaves the mode. Pasting text that starts with a prefix enters that mode.

| Prefix | Mode |
| --- | --- |
| `>` | Commands: system commands, math, units, web shortcuts, ports, media, capture |
| `!` | Apps and programs |
| `/` | Find files and folders (plain-language too) |
| `?` | Ask your AI providers |

| Key | Action |
| --- | --- |
| `Alt+Space` | Show or hide Flick (configurable) |
| `Up` / `Down` | Move the selection |
| `Enter` | Primary action (launch, open, copy result, ask) |
| `Ctrl+Enter` | Open file location (Find, Apps) |
| `Ctrl+Shift+Enter` | Run as admin (Apps) |
| `Ctrl+C` | Copy path (Find) or copy answer (Ask), only when no text is selected |
| `Tab` / `Shift+Tab` | Next / previous thinking mode (Ask) |
| `Ctrl+T` | Fast/Think toggle (Ask) |
| `Ctrl+Right` / `Ctrl+Left` | Next / previous tutorial step |
| `Alt+T` | Try the tutorial example |
| `F1` | Open the tutorial |
| `Esc` | Cancel a running answer, then clear the input, then hide (skips the tutorial while it is open) |

Flick hides on `Esc`, on focus loss, and after a successful launch.

## Features

- **Programs:** `!` also lists programs found by scanning every fixed drive, ranked below installed apps. [Details](docs/features/programs.md).
- **Thinking modes:** in `?`, every enabled provider and model is a mode; Tab switches. [Details](docs/features/thinking-modes.md).
- **Themes:** Glass, Frost, Midnight, Aurora, Ember and Solid, in Settings, Appearance. [Details](docs/features/appearance.md).
- **Tutorial:** a skippable tour on first open; replay with `F1`. [Details](docs/features/tutorial.md).
- **Frecency:** ranks items you use often and recently higher, with a 7-day half-life. You can turn on prefix learning, change the half-life, clear history, or turn ranking off to get pure match order.
- **Math and units:** type arithmetic, units, currency or time zones in Commands mode (`>`), for example `2.5 kg to lb`, `250 usd in inr` or `5pm ist in pst`. Enter copies the result. Currency uses the ECB daily reference rates, fetched at most once a day and cached. When offline, Flick uses the last cached rate and shows its age. The rates are published by the European Central Bank.
- **System commands:** Lock, Sleep, Hibernate, Restart, Shut down, Sign out, Empty Recycle Bin, Restart Explorer, Wi-Fi on/off and Bluetooth on/off. Restart, Shut down, Sign out and Empty Recycle Bin need Enter twice within 3 seconds.
- **Web shortcuts:** a keyword plus a query opens a URL. Defaults: `g` (Google), `yt` (YouTube), `gh` (GitHub), `amz` (amazon.in), `maps` (Google Maps), `wiki` (Wikipedia). Add, edit, remove and reorder them in Settings.
- **Project launcher:** folders with `.git`, `package.json`, `Cargo.toml`, `pyproject.toml` or `go.mod` under your dev roots, plus recent VS Code workspaces. `code <name>` opens the editor (VS Code by default; Cursor and Zed are selectable), `term <name>` opens Windows Terminal there, and a bare name opens the folder.
- **Natural-language finder:** in Find, `the invoice i downloaded last week` is parsed offline and opens only a clear winner of a safe file type; otherwise it lists the candidates. Decision engines (Laya, Kev, your AI provider, Jev) are optional and off by default. [Details](docs/features/natural-language-finder.md).
- **Ports and processes:** `port 3000` shows what holds a port, `proc node` searches processes. Arrow to a row, press `Backspace`, then `Enter` to end it. [Details](docs/features/ports-and-processes.md).
- **Media controls:** `play`, `pause`, `next`, `previous`, `now playing` work on any player. `play X on spotify` / `on youtube` open a search; connecting your own Spotify developer app is optional. [Details](docs/features/media-controls.md).
- **Explain this:** a second hotkey (`Alt+Shift+Space`) captures the selected text and shows it in Ask mode (`?`) before anything is sent; secret-looking text is blocked. [Details](docs/features/explain-this.md).
- **Quick capture:** `log 250 lunch #food`, `note ...`, `todo ...` go to an Obsidian daily note, a Markdown file, Notion or a webhook. [Details](docs/features/quick-capture.md).

## AI providers

Flick has no default provider. The presets, in fallback order, are:

- **Local:** Ollama (`http://localhost:11434/v1`), LM Studio (`http://localhost:1234/v1`), llama.cpp server (`http://localhost:8080/v1`)
- **Free gateways:** OmniRoute (`http://localhost:20128/v1`), OpenRouter (`https://openrouter.ai/api/v1`)
- **Your own key:** Anthropic, OpenAI, Custom (any OpenAI-compatible base URL, model and optional key)

On first run, and on demand, Flick looks for the three local ports and lists the models it finds. It offers them but never switches one on for you.

Local setup:

- **Ollama:** run `ollama serve`, then `ollama pull <model>`.
- **LM Studio:** start the local server from LM Studio.
- **llama.cpp:** run `llama-server` with a model.

Ask uses the thinking mode you chose first (see [thinking modes](docs/features/thinking-modes.md)), then the first enabled provider that responds. On a connection error, 429 or 5xx, it falls back down your order.

## Privacy and security

- Provider keys are stored in Windows Credential Manager. They are write-only from the UI: no screen or command ever shows a stored key again. Flick shows only the last four characters, the host, and when the key was last used.
- A key is bound to the host it was entered for and is sent only over HTTPS to that host (plain HTTP is allowed only for localhost and 127.0.0.1).
- Every "Add key" dialog shows a risk warning, and Save stays disabled until you tick the box confirming you understand the risk and have set a usage limit.
- All network calls are made by the Rust side. The WebView makes no network calls, and its Content Security Policy blocks remote scripts, frames and images.
- Model output is untrusted. It is rendered as sanitized Markdown with raw HTML off, and it cannot trigger any action by itself.
- Text that looks like a secret is blocked from being sent to a model.
- No telemetry, and no network calls on the keystroke path.

See [ADR-011](docs/adr/ADR-011-secret-security.md) for the full rules.

## Build from source

Prerequisites:

- Rust stable with the `x86_64-pc-windows-msvc` target (MSVC Build Tools)
- Node.js and pnpm (the workspace pins pnpm 10.28.1)
- Microsoft Edge WebView2 runtime

Commands:

```
pnpm install
pnpm --filter desktop tauri dev        # run the desktop app
cargo test -p <crate>                  # run tests for one crate
cargo clippy --all-targets -- -D warnings
cargo fmt --check
pnpm --filter desktop typecheck
pnpm --filter desktop test
pnpm lint
pnpm --filter web dev                  # landing site
```

## Performance

Measured on the dev machine: launch call 0 to 1 ms (budget 30 ms), command match across 2,000 apps 0.25 ms (budget 2 ms), and Find over about 305,000 paths p95 4.7 ms or less (budget 25 ms). The budgets and the full measurements are in [docs/performance.md](docs/performance.md).

## License

MIT. See [LICENSE](LICENSE).

## Architecture and decisions

Design decisions are recorded in [docs/adr/](docs/adr/):

- [ADR-001](docs/adr/ADR-001-shell-and-ui.md) Shell and UI
- [ADR-002](docs/adr/ADR-002-app-index.md) App index
- [ADR-003](docs/adr/ADR-003-file-index.md) File index
- [ADR-004](docs/adr/ADR-004-matching.md) Matching
- [ADR-005](docs/adr/ADR-005-natural-language-finder.md) Natural-language finder
- [ADR-006](docs/adr/ADR-006-providers.md) Providers
- [ADR-007](docs/adr/ADR-007-storage-and-secrets.md) Storage and secrets
- [ADR-008](docs/adr/ADR-008-repo-and-web.md) Repo and web
- [ADR-009](docs/adr/ADR-009-feature-modules.md) Feature modules and toggles
- [ADR-010](docs/adr/ADR-010-integrations.md) Integrations
- [ADR-011](docs/adr/ADR-011-secret-security.md) Secret security
