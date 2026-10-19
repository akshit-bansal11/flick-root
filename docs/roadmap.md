# Roadmap

Flick is a free, open-source, keyboard-first launcher for Windows. This page lists what is planned. It does not promise dates. The decisions behind each part are in [docs/adr/](adr/), and the README lists what is built today.

## v0.0.1: core (in progress)

The first release covers the core launcher:

- A global hotkey (Alt+Space, configurable) that shows and hides the launcher.
- Command mode: fuzzy search of installed apps, including Store apps, with launch, run as admin and open location.
- Find mode: fuzzy search of files and folders under the indexed roots, with open, open file location and copy path.
- Query mode: streams an answer from the first enabled provider that responds, with Fast/Think and provider fallback.
- Modes chosen by the `>`, `!`, `/` and `?` prefixes (v0.1.0 also had Tab and Shift+Tab; removed in v0.2.0).
- Feature toggles for frecency ranking, inline math and units, system commands, web shortcuts and the project launcher.
- A Settings window and a first-run flow.
- A tray icon with Open, Settings and Quit, single-instance behaviour, and a launch-at-startup toggle.

## v0.1.0: integrations

The second release adds integrations:

- Natural-language finder (ADR-005).
- Ports and processes.
- Media controls with optional Spotify and YouTube connections (ADR-010).
- Explain this, which sends the captured text to a provider you chose after showing it and blocking secret-looking text.
- Quick capture to Obsidian, a Markdown file, Notion and webhooks (ADR-010).

Integrations that need an account are optional. No feature may require a paid account.

## Known gaps

Known gaps and deferred review notes are tracked in [docs/followups.md](followups.md). Items there are deferred on purpose, and each entry names where the gap is.
