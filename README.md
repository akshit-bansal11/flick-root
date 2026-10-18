# Flick

Flick is a free, open-source, keyboard-first launcher for Windows. Command, Find and Query modes share one bar: open an app, find a file, or ask a model, without leaving the keyboard. It is a wrapper. It ships no models, no API keys, no account and no default provider, and nothing AI-related is switched on out of the box. You choose every provider, and Command and Find work fully with no AI at all. There is no telemetry, no sign-in, and the license is MIT.

## Status

Flick is v0.0.1 in progress. Built so far:

- Command: fuzzy search of installed apps, including Store apps, as Start's "All apps" lists them.
- Find: fuzzy search of files and folders under the indexed roots, with Open, Open file location and Copy path.
- Query: streams an answer from the first enabled provider that responds, with Fast/Think and provider fallback.
- Features: frecency, inline math and units, system commands, web shortcuts, project launcher.
- Settings window (General, Features, AI providers, About) and first run.

Planned for v0.1.0: Natural-language finder, ports and processes, media controls, Explain this, and Quick capture.

## Install

Download the installer from the GitHub Releases page of [akshit-bansal11/flick](https://github.com/akshit-bansal11/flick/releases) once a release is published. The repository is currently private.

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC". Choose **More info**, then **Run anyway**. Each release publishes SHA-256 checksums so you can verify the download.

## Hotkeys and modes

| Key | Action |
| --- | --- |
| `Alt+Space` | Show or hide Flick (configurable) |
| `Tab` / `Shift+Tab` | Cycle modes |
| `>` / `?` as the first character | Switch to Command / Query (`/` switches to Find); the character is removed |
| `Up` / `Down` | Move the selection |
| `Enter` | Primary action (launch, open, copy result, ask) |
| `Ctrl+Enter` | Open file location (Find, Command) |
| `Ctrl+Shift+Enter` | Run as admin (Command) |
| `Ctrl+C` | Copy path (Find) or copy answer (Query), only when no text is selected |
| `Ctrl+T` | Fast/Think toggle (Query) |
| `Ctrl+P` | Next provider (Query) |
| `Esc` | Cancel a running answer, then clear the input, then hide |

Flick hides on `Esc`, on focus loss, and after a successful launch.

## Features

- **Frecency:** ranks items you use often and recently higher, with a 7-day half-life. You can turn on prefix learning, change the half-life, clear history, or turn ranking off to get pure match order.
- **Math and units:** type arithmetic, units, currency or time zones in Command mode, for example `2.5 kg to lb`, `250 usd in inr` or `5pm ist in pst`. Enter copies the result. Currency uses the ECB daily reference rates, fetched at most once a day and cached. When offline, Flick uses the last cached rate and shows its age. The rates are published by the European Central Bank.
- **System commands:** Lock, Sleep, Hibernate, Restart, Shut down, Sign out, Empty Recycle Bin, Restart Explorer, Wi-Fi on/off and Bluetooth on/off. Restart, Shut down, Sign out and Empty Recycle Bin need Enter twice within 3 seconds.
- **Web shortcuts:** a keyword plus a query opens a URL. Defaults: `g` (Google), `yt` (YouTube), `gh` (GitHub), `amz` (amazon.in), `maps` (Google Maps), `wiki` (Wikipedia). Add, edit, remove and reorder them in Settings.
- **Project launcher:** folders with `.git`, `package.json`, `Cargo.toml`, `pyproject.toml` or `go.mod` under your dev roots, plus recent VS Code workspaces. `code <name>` opens the editor (VS Code by default; Cursor and Zed are selectable), `term <name>` opens Windows Terminal there, and a bare name opens the folder.

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

Query uses the first enabled provider that responds. On a connection error, 429 or 5xx, it falls back down your order.

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
