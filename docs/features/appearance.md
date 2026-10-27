# Appearance

The launcher and Settings are flat and opaque: no acrylic, Mica, blur, gradients or glass. The
launcher is an 820px panel with a 1px line edge. Its height follows the results. Text is set in
IBM Plex Mono for the interface and IBM Plex Sans for longer prose.

## The launcher

- A mode tab strip sits at the top: Open, Commands, Calc, Ask and Projects. `ctrl+tab` and
  `ctrl+shift+tab` move between tabs.
- Two panes: the result list on the left, with `#` group headers, and a detail pane on the right
  that lists every action for the selected row.
- A status line at the bottom shows the state and the keys that apply.
- In Ask, models and effort are on the left and the answer is on the right. The answer scrolls.

## Themes

A theme is a set of colours resolved in Rust (`crates/launcher-core/src/theme/`). The seed gives
the background, panel, text, muted text and accent, with an optional selection, line and surface
colour. Flick derives the rest and lifts any text colour that does not reach 4.5:1 contrast. A
seed that cannot reach 4.5:1 is refused, not shown. The derivation and its corrections are in
[ADR-015](../adr/ADR-015-flat-ui-and-themes.md).

- **Built in:** the Flick Default themes (dark and light), and frost, midnight, aurora, ember, sage,
  paper, harbor, lagoon and rose, with light variants for sage, paper, harbor, lagoon and rose.
- **Ported editor themes:** 40 at this commit, each under an MIT, ISC, BSD or CC0 licence. The
  upstream repo, licence and copyright for each are in
  [THIRD-PARTY-THEMES.md](../../THIRD-PARTY-THEMES.md).
- **Import a VS Code theme:** in Settings, Appearance, `import VS Code theme…` opens the Windows file
  picker for a `.json` or `.jsonc` colour theme. The file is read as untrusted input: at most 1 MiB,
  JSONC comments and trailing commas allowed, `include` never followed, safe names. A preview shows
  each Flick role, the VS Code key it came from, the colour, and whether it was used as is, raised for
  contrast or filled from a fallback. `import as "<name>"` saves it as an `imported` theme; a selected
  imported theme can be deleted (with a confirm step).

## Settings, Appearance

- **Scheme:** System, Dark or Light. System follows Windows.
- **Dark theme** and **Light theme:** one slot each. Change opens a filterable list with a live
  preview. Saving a theme in one slot applies it when the scheme matches that slot.

The choice is stored under `[appearance]` in `config.toml` as `scheme`, `dark_theme` and
`light_theme`. An absent slot uses the Flick default for that scheme. The old `theme` key is
migrated once on load: `frost` becomes the light theme, and `midnight`, `aurora` and `ember` become
the dark theme. `glass` and the glass strength are no longer used and are removed on the next save.

## Theme quick pick

In Commands, type `theme` to list every theme. Arrowing through the list previews each theme on the
launcher. Enter saves the theme to its slot and sets the scheme to match it. `ctrl+enter` saves it to
its slot only and leaves the scheme as it is. The first Escape puts the saved look back.

## Contrast

`apps/desktop/src/tokens.contrast.test.ts` checks the CSS roles. `crates/launcher-core/src/theme/`
checks every built-in and imported theme at 4.5:1 for text, muted text, accent text, the selected
row and the text on accent. `apps/desktop/src/tokens.hygiene.test.ts` fails on gradients, blur,
inset highlights and glass leftovers.

The choice is applied to the launcher every time it is shown.
