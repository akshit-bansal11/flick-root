# ADR-015: Flat UI and themes

Status: Accepted (2026-10-25). Supersedes the glass and backdrop parts of ADR-013 and ADR-014.

## Context

The launcher used an acrylic backdrop, glass tints, blur, gradients and inset highlights, with a
"lens" that slid between rows. Settings used Mica behind the window. Themes were a fixed list of
CSS blocks, with a glass strength slider and a single `theme` key. The look cost readability and
GPU work, and it did not match the keyboard-first, text-first behaviour of the app. Contrast was
checked for a few hand-written palettes only, and users could not bring their own colours.

## Decision

- **Flat.** The launcher and Settings are opaque. No acrylic, Mica, blur, gradients or inset
  highlights. The panel is `--l-panel` with a 1px `--l-line` edge. Corners follow the window radius
  (8px). `tokens.hygiene.test.ts` fails the test run on `gradient(`, `backdrop-filter`,
  `mask-composite`, inset box shadows, `Lens` or `LiquidFilter` imports, and `data-glass`.
- **Type.** Chrome uses IBM Plex Mono (`--l-ui`). Prose, such as the Ask answer and long help text,
  uses IBM Plex Sans (`--l-prose`). Body size is 13px.
- **Launcher layout.** 820px wide (`PANEL_W` in `window.rs`, `--l-panel-w` in `tokens.css`). A mode
  tab strip (Open, Commands, Calc, Ask, Projects), then two panes: the list on the left and the
  detail pane on the right, then a status line. Ask shows models and effort on the left and the
  answer on the right. Keyboard shortcuts are shown as lowercase words, for example `ctrl+tab`.
- **Shortcuts.** `ctrl+tab` and `ctrl+shift+tab` switch mode tabs. `ctrl+p` pins or unpins in Home
  and Open. Typing `theme` in Commands opens the theme quick pick, which previews live. The table is
  `apps/desktop/src/shortcuts.ts`.
- **Themes are resolved in Rust.** `crates/launcher-core/src/theme/` turns a seed (`bg`, `panel`,
  `text`, `muted`, `accent`, optional `sel`, `line`, `surface`) into thirteen roles: `bg panel
  surface line text muted accent accentText onAccent sel ok warn err`. The UI only paints them.
  The derivation ports the editor-theme derivation with three corrections:
  1. A lift picks its pole by comparing the real ratios of white and black on the backgrounds. The
     fixed "dark or light" test cannot reach 4.5:1 on mid-tone panels.
  2. `onAccent` tries `bg`, `panel`, `#ffffff`, `#111111` and `#000000`. `#111111` alone has a worst
     case of 4.35:1.
  3. `muted` and `accentText` are lifted against panel, bg and sel, and `text` against panel and bg,
     because the tab strip, status line and nav sit on bg and tags sit on sel.
  A seed `sel` closer than 1.1:1 to the panel is ignored and derived. The derived colour is tinted
  toward the panel in steps until `text` reaches 4.5:1 on it. Status colours are lifted to 4.5:1.
  A theme whose text, muted, accent, sel or `onAccent` check still fails is rejected with
  `ThemeError::Unreadable`. The built-in registry fails on it, and an imported theme is dropped.
  Every built-in theme is resolved, so contrast-checked, each time the registry is built.
- **Built-in themes.** Two defaults, built in code (`brand.rs` holds the ids `flick-default` and
  `flick-default-light`), and the Flick seeds in `seeds/flick.json`: frost, midnight, aurora, ember,
  sage, paper, harbor, lagoon and rose, the last five with `-light` variants.
- **Ported editor themes.** `seeds/ports.json` holds 40 ports at this commit. Each port records its
  upstream repo, theme file, licence file and copyright in `THIRD-PARTY-THEMES.md`. A licence must
  be MIT, ISC, BSD-2-Clause, BSD-3-Clause, 0BSD or CC0-1.0, confirmed from the upstream LICENSE
  file. Apache-2.0, GPL, CC-BY-SA, unclear and paid themes are not ported. A Rust test checks the
  table against the seeds and the allowed SPDX set. The list of excluded themes is in the same file.
- **Config.** `[appearance]` has `dark_theme` and `light_theme` (absent means the default for that
  scheme) and `scheme`. The old `theme` key is migrated once on load: `frost` becomes the light
  theme, and `midnight`, `aurora` and `ember` become the dark theme. A `system` scheme becomes the
  forced scheme so the look is kept. `glass` and the glass strength are dropped. `[home] pins` holds
  up to 24 ids, each `app:` or `file:`.
- **Imported themes are untrusted input.** The VS Code parser (`theme/vscode.rs`) caps a file at
  1 MiB, strips JSONC comments and trailing commas in one pass, never follows `include`, caps names
  at 80 characters and slugs at 40, and never produces a bare Windows device name such as `con` or
  `com1`. Imported ids are `imported:<slug>`.

## Importing without a dialog plugin

Settings picks the file with a native `<input type="file">` and reads it with the File API, so no
dialog dependency is added and no path reaches the WebView (the File API exposes only a name). The
page refuses a file over 1 MiB before reading it; `preview_vscode_theme` checks the size again before
parsing and holds the result behind a single-use token. `save_imported_theme` writes
`themes/<slug>.json` in the config folder atomically, and the registry re-checks every saved file on
load (at most 100 files, 8 KB each). `delete_imported_theme` accepts only `imported:<slug>` ids. The
three commands exist only in the Settings window.

## Alternatives

- Keep glass and add a "solid" option. Rejected: two looks double the contrast work and the
  hygiene surface, and the flat look is the one that was reviewed.
- Compute themes in the UI. Rejected: the contrast rules then live in TypeScript and are tested
  only in the browser. In Rust they run in `cargo test` and the Rust side can reject a theme before
  it reaches a window.
- Import themes by fetching from a URL. Rejected: the WebView makes no network calls, and the
  launcher should not reach the network from a theme file.

## Consequences

- Contrast is enforced by the resolver for every built-in and imported theme, not by a review of each
  palette. A seed that cannot reach 4.5:1 is refused rather than shown.
- Old config files keep their look once. A file with `glass` loads, and the key is removed on the
  next save.
- CI covers the derivation, the registry, the migration and the hygiene rules. CI cannot prove
  how the flat panel looks on real displays, the caret style on a given machine's WebView2 version, or
  contrast over real wallpapers. Those need a manual pass on Windows before release.
