# Flick

Flick is a free, open-source, keyboard-first launcher for Windows. Open, Commands, Calc, Ask and Projects share one bar, each behind a one-character prefix: open an app, program, file or folder, run a command, calculate, ask a model, or jump into a project, without leaving the keyboard. It is a wrapper. It ships no models, no API keys, no account and no default provider, and nothing AI-related is switched on out of the box. You choose every provider, and everything except Ask works fully with no AI at all. There is no telemetry, no sign-in, and the license is MIT.

## Status

Flick is at v0.0.1. Features:

- Open (`/`): fuzzy search of installed apps, including Store apps, as Start's "All apps" lists them, plus programs found on your drives, and files and folders under the indexed roots, with Open, Open file location and Copy path.
- Commands (`>`): system commands, web shortcuts, ports, media, capture.
- Calc (`=`): math, units, currency and time zones.
- Ask (`?`): streams an answer from the provider and model you pick, with Fast/Think, thinking modes and provider fallback.
- Projects (`@`): code folders under your dev roots, most used first.
- Features: frecency, inline math and units, system commands, web shortcuts, project launcher.
- Settings window (General, Appearance, Prefixes, AI providers, Decision engine, Integrations, About), first run with a welcome screen, and an interactive tour.
- Natural-language finder, ports and processes, media controls, Explain this, and Quick capture (see Features below and [docs/features/](docs/features/)).
- Programs on all drives, thinking modes, and a flat launcher with mode tabs, two panes and a status line.
- A flat tray menu (Open, Settings, updates, Quit) that opens from a right-click on the tray icon, in the same window as the launcher.
- One shortcut table for the launcher and Settings (below), an interactive tour in its own window, and a tutorial.

## Install

Download the installer from the GitHub Releases page of [akshit-bansal11/flick](https://github.com/akshit-bansal11/flick/releases) once a release is published.

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC". Choose **More info**, then **Run anyway**. Each release publishes SHA-256 checksums so you can verify the download.

## Hotkeys and modes

A prefix typed first in the empty bar narrows the search to one set of results; `ctrl+tab` and `ctrl+shift+tab` move between mode tabs. Plain text with no prefix searches apps, programs and every enabled feature (not files), and ends with up to three "Search instead" rows: Find files named your text (enters `/`), search the web with your first web shortcut, and Ask. A prefix is offered only while a feature it needs is on. Backspace on an empty input leaves the mode. Pasting text that starts with a prefix enters that mode. With `@` and nothing after it, Flick lists your projects, most used first (the order comes from Frecency ranking; with Frecency off it is scan order).

| Prefix | Mode | Sources |
| --- | --- | --- |
| `/` | Open | Apps, programs, files and folders (plain-language file requests too), grouped |
| `>` | Commands | System commands, web shortcuts, media, ports and processes, quick capture |
| `=` | Calc | Math, units, currency and time zones |
| `?` | Ask | Your AI providers (never searches) |
| `@` | Projects | Code folders under your dev folders |

| Key | Action |
| --- | --- |
| `Alt+Space` | Show or hide Flick (configurable). It replaces Windows' window menu; the hotkey picker warns about it. |
| `Up` / `Down` | Move the selection |
| `Enter` | Primary action (open, launch, ask) |
| `Ctrl+Enter` | Show location (Open) |
| `Ctrl+Shift+Enter` | Run as admin (Open: apps and exe/msi programs, when the result can) |
| `Ctrl+Shift+C` | Copy: path in Open (files and folders), result in Commands and Calc, answer in Ask |
| `Delete` | End a process, after arrowing to its row in Commands; `Enter` confirms |
| `Ctrl+1` to `Ctrl+5` | Go to Open, Commands, Calc, Ask, Projects |
| `Tab` / `Shift+Tab` | Next / previous thinking mode (Ask) |
| `Ctrl+T` | Fast/Think toggle (Ask) |
| `Ctrl+Shift+X` | Remove the attached Explain text (Ask) |
| `Backspace` | On an empty input, leave the mode |
| `Esc` | Cancel a running answer, then clear the input, then hide |
| `Ctrl+,` | Open Settings |
| `F1` or `Ctrl+/` | Shortcut list |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous page (Settings) |
| `Esc` or `Ctrl+W` | Close Settings |
| `PageDown` / `PageUp` | Next / previous tour step |

Text-editing chords (such as `Ctrl+C`, `Ctrl+V`, `Ctrl+Backspace`, `Shift+Arrow`) and browser/WebView chords (such as `Ctrl+F`, `Ctrl+R`, `F5`, `F12`) are reserved: Flick never uses them, and the launcher and tour cancel the browser action of the browser ones.

Flick hides on `Esc`, on focus loss, and after a successful launch.

## Features

- **Programs:** `/` also lists programs found by scanning every fixed and removable drive, ranked below installed apps; loose fuzzy hits are dropped. [Details](docs/features/programs.md).
- **Search:** typos are tolerated in app names, system commands and feature keywords (`spotifu`, `slep`); file search matches letters in order. [Details](docs/features/search.md).
- **Thinking modes:** in `?`, every enabled provider and model is a mode; Tab switches. [Details](docs/features/thinking-modes.md).
- **Themes:** Flick's own themes and ported editor themes (MIT, ISC, BSD or CC0 licences), each checked for contrast. Pick one for dark and one for light in Settings, Appearance. Importing a VS Code theme is not built yet. [Details](docs/features/appearance.md).
- **Pins:** `ctrl+p` pins or unpins an app or file in Home and Open; pinned items sit at the top. [Details](docs/features/search.md). [Details](docs/features/appearance.md).
- **Welcome and tour:** first launch asks to take an interactive tour; replay it in Settings > General. [Details](docs/features/tutorial.md).
- **Frecency:** ranks items you use often and recently higher, with a 7-day half-life. You can turn on prefix learning, change the half-life, clear history, or turn ranking off to get pure match order.
- **Math and units:** type arithmetic, units, currency, time zones or number bases after `=`, for example `2.5 kg to lb`, `250 usd in inr`, `5pm ist in pst`, `255 to hex` or `0xff to decimal`. Enter copies the result. Currency uses the ECB daily reference rates, fetched at most once a day and cached. When offline, Flick uses the last cached rate and shows its age. The rates are published by the European Central Bank.
- **System commands:** Lock, Sleep, Hibernate, Restart, Shut down, Sign out, Empty Recycle Bin, Restart Explorer, Wi-Fi on/off and Bluetooth on/off. Restart, Shut down, Sign out and Empty Recycle Bin need Enter twice within 3 seconds.
- **Web shortcuts:** a keyword plus a query opens a URL. Defaults: `g` (Google), `yt` (YouTube), `gh` (GitHub), `amz` (amazon.in), `maps` (Google Maps), `wiki` (Wikipedia). Add, edit, remove and reorder them in Settings.
- **Links and paths:** type an http or https link, a bare domain such as `github.com/x`, or a drive path such as `C:\Users\me\Docs`, and Flick adds an Open row. It opens only when you press Enter, never automatically. Works in plain text and after `>`. [Details](docs/features/links-and-paths.md).
- **Project launcher (`@`):** folders under your dev roots that hold `.git` or a common project file (for example `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `requirements.txt`, `pom.xml`, `build.gradle`, `composer.json`, `Gemfile`, `Makefile`, `CMakeLists.txt`, `deno.json`, `pubspec.yaml`, `mix.exs`, or a `.csproj`, `.sln` or `.cabal` file), plus recent VS Code workspaces. `code <name>` opens the editor (VS Code by default; Cursor and Zed are selectable), `term <name>` opens Windows Terminal there, and a bare name opens the folder. `@` with nothing after it lists projects, most used first (the order comes from Frecency ranking; with Frecency off it is scan order).
- **Natural-language finder:** after `/`, `the invoice i downloaded last week` is parsed offline and opens only a clear winner of a safe file type; otherwise it lists the candidates. Decision engines (Laya, Kev, your AI provider, Jev) are optional and off by default. [Details](docs/features/natural-language-finder.md).
- **Ports and processes:** after `>`, `port 3000` shows what holds a port, `proc node` searches processes. Arrow to a row, press `Delete`, then `Enter` to end it. [Details](docs/features/ports-and-processes.md).
- **Media controls:** after `>`, `play`, `pause`, `next`, `previous`, `now playing` work on any player. `play X on spotify` / `on youtube` open a search; connecting your own Spotify developer app is optional. [Details](docs/features/media-controls.md).
- **Explain this:** a second hotkey (`Alt+Shift+Space`) captures the selected text and shows it in Ask (`?`) before anything is sent; secret-looking text is blocked. [Details](docs/features/explain-this.md).
- **Quick capture:** after `>`, `log 250 lunch #food`, `note ...`, `todo ...` go to an Obsidian daily note, a Markdown file, Notion or a webhook. [Details](docs/features/quick-capture.md).

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
- Update check: at most once a day Flick asks api.github.com for the latest release version. Turn it off in Settings › General. Install downloads the installer from GitHub, checks its signature against the key built into Flick, and runs it.
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

Ported editor themes keep their upstream licences and copyright; see [THIRD-PARTY-THEMES.md](THIRD-PARTY-THEMES.md).

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
- [ADR-012](docs/adr/ADR-012-prefixes-and-plain-text.md) Prefixes and plain text
- [ADR-013](docs/adr/ADR-013-tray-menu.md) Tray menu
- [ADR-014](docs/adr/ADR-014-settings-backdrop.md) Settings backdrop
- [ADR-015](docs/adr/ADR-015-flat-ui-and-themes.md) Flat UI and themes (supersedes the glass parts of ADR-013 and ADR-014)
