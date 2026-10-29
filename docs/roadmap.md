# Roadmap

Flick is a free, open-source, keyboard-first launcher for Windows. This page lists the scope of the current version. It does not promise dates. The decisions behind each part are in [docs/adr/](adr/), and the README lists what is built today.

## v0.0.1

- A global hotkey (Alt+Space, configurable) that shows and hides the launcher.
- Open (`/`): fuzzy search of installed apps (including Store apps), programs, files and folders, with launch, run as admin, open location and copy path.
- Commands (`>`): system commands, web shortcuts, media, ports and capture.
- Calc (`=`): math, units, currency and time zones.
- Ask (`?`): streams an answer from the first enabled provider that responds, with Fast/Think and provider fallback.
- Modes chosen by the `/`, `>`, `=`, `?` and `@` prefixes. A prefix is offered only while a feature it needs is on.
- Feature toggles for frecency ranking, inline math and units, system commands, web shortcuts.
- Integrations: natural-language finder (ADR-005), ports and processes, media controls with optional Spotify and YouTube connections (ADR-010), Explain this, and Quick capture to Obsidian, a Markdown file, Notion and webhooks (ADR-010).
- A Settings window, a first-run flow, and a tray icon with Open, Settings and Quit, single-instance behaviour, and a launch-at-startup toggle.

Integrations that need an account are optional. No feature may require a paid account.

## Known gaps

Known limits are stated in the ADRs and in the feature pages under [docs/features/](features/).
