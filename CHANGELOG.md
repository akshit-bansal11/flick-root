# Changelog

All notable changes to Flick are documented here.

## [1.0.0] - 2026-10-27

### Added

- Five prefixes in one bar: `/` Open (apps, programs, files and folders), `>` Commands, `=` Calc, `?` Ask and `@` Projects. A prefix appears only while a feature it needs is on.
- Plain text searches apps, programs and every enabled feature, then offers Search instead rows: find files with your text, search the web with your first web shortcut, or Ask.
- Typed links (http and https) and drive paths get an Open row. Nothing opens until you press Enter, and a typed path opens only folders and safe document types.
- Mode tabs above the results; `ctrl tab` and `ctrl shift tab` move between them.
- The empty bar lists your recent picks. Pin apps, files and folders with `ctrl p` (up to 24); pins show first, under `# pinned`.
- A detail pane for the selected row with every action and its keys, and a status line with result counts and key hints.
- Themes with separate dark and light picks: Flick's own themes plus ported editor themes such as One Dark Pro, Dracula, Nord, Gruvbox, Solarized, Catppuccin, Tokyo Night, GitHub, Rosé Pine and Ayu. Every theme is checked for 4.5:1 text contrast. Upstream authors and licences are listed in THIRD-PARTY-THEMES.md.
- `> theme` in the bar previews themes as you move through the list.
- Import a VS Code colour theme file from Settings, Appearance, with a preview of where each colour comes from and whether it was raised for contrast.
- Right-click the tray icon for a menu with the version and update status.
- `@` recognises more project types: Python requirements, .NET, Java, Gradle, PHP, Ruby, CMake, Make, Deno, Flutter, Elixir and Haskell.
- `=` converts number bases, for example 255 to hex.

### Changed

- A flat look: monospace chrome, plain panels and one-pixel lines, with no glass, blur or gradients. The launcher is wider, with results on the left and details on the right, and keys are written as words.
- Settings pages are rebuilt as rows with on and off switches; each row explains what it does.
- First run shows its steps and progress and offers detected local AI servers as a single choice; skip keeps Ask off.
- The tour teaches the current launcher.
- Run as admin and Copy path appear only on results that support them.

### Removed

- The `!` prefix: apps are found by plain text and under `/`.
- The glass strength setting and the single theme setting. An existing theme choice moves to the new dark and light picks.

### Fixed

- Keyboard focus stays on the replacing button when a button swaps (clear and confirm, delete and keep), and buttons that repeat on a page are named for their item.
- Dependency security update (ctutils 0.4.4).

## [0.0.1] - 2026-10-22

### Added

- Keyboard-first launcher for Windows with a global hotkey (Alt+Space, configurable) and a bar that shows results under it.
- Prefix-chosen modes: apps (`!`), commands (`>`), Find (`/`) and Ask (`?`).
- App index covering installed apps, including Store apps, with launch, run as admin and open location, plus programs found on your drives.
- Find mode: fuzzy search of files and folders under the indexed roots, with open, open file location and copy path.
- Inline math, unit conversion, time zones and currency conversion from ECB reference rates.
- Project launcher, web shortcuts, system commands with a double-Enter guard, and frecency ranking.
- AI providers you choose (OpenAI-compatible endpoints, Anthropic and local engines), with Fast and Think modes and provider fallback. Keys live only in Windows Credential Manager.
- Natural-language file finder that parses offline.
- Media controls for any Windows media player, with optional Spotify and YouTube links.
- Ports and processes: `port <n>` and `proc <name>`.
- Explain this: a second hotkey sends the selected text to your provider after showing it and blocking secret-looking text.
- Quick capture to Obsidian, a Markdown file, Notion or a webhook.
- Six themes, an acrylic launcher window, a Settings window, a first-run welcome screen and an interactive tour.
- Update notification in the tray, with a signed self-installing update.
