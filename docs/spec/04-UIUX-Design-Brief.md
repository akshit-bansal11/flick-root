# Flick: UI/UX Design Brief

| Field | Value |
|---|---|
| Document | UI/UX Design Brief |
| Version | 1.0 |
| Date | 19 Oct 2026 |
| Owner | Akshit Bansal |
| Status | Draft |

## What this document is for

This brief tells designers and developers how Flick looks, feels and behaves. It turns the chosen Design Lab configuration into tokens, sizes, components and rules. Every value comes from the Design Lab file or is marked **Decision:** with a reason. Where the 19 Oct security update changes the UI (the Add key dialog, masked keys, the Query empty state), this brief follows it.

## Summary

- Direction: Glacier palette, Solid material, Compact strip layout, Hint only mode switcher, IBM Plex Sans and Mono, Sharp corners, Comfortable density.
- Dark and light themes follow Windows. Colors, radius, row height and fonts live only in `tokens.css`.
- The launcher is a 700 px strip with 44 px rows. It grows only for the Find action row and the Query answer (up to 60% of monitor height).
- Nothing animates on a keystroke. The show animation is 100 ms and respects reduced motion.
- Secrets are write-only in the UI: a warning dialog with a required checkbox, then a masked display.

## 1. Design goals and principles

| Goal | What it means in practice |
|---|---|
| Speed you can feel | The window is prewarmed, so it only shows and hides. Hotkey to visible in 50 ms or less. A keystroke paints in 16 ms or less. No spinner on a keystroke. |
| Keyboard first | Every action has a key. The mouse works but is never required. Focus never leaves the input in the launcher. |
| Calm and readable | One accent color. Opaque panel. No gradients, glass or glow. Text is never smaller than needed and never low in contrast. |
| FOSS honesty in copy | Say what Flick does and does not do. No "free forever", no hype. Say where data goes. Say that no storage is completely safe. |

## 2. The chosen direction and why it fits

| Choice | Value | Why it fits |
|---|---|---|
| Palette | Glacier | Ice cyan on deep slate. Cold, quiet, very readable. Suits a tool used all day. |
| Material | Solid | Opaque background, no window effect. Fastest to render and contrast does not depend on the wallpaper. |
| Layout | Compact strip | Thin bar, up to 5 one-line results, no footer. Closest to `Win+R`. Takes little space. |
| Mode switcher | Hint only | No control. The input shows `Command · Tab`. Cleanest. The first-run overlay teaches `Tab` and the `>` `/` `?` prefixes. |
| Type | IBM Plex Sans and Mono | Engineered and clear at small sizes. Mono for keys, paths and status. |
| Corners | Sharp | 6 px panel, 3 px rows. Precise, matches the technical tone. |
| Density | Comfortable | 44 px rows. Easy to hit and read. |

The hint-only switcher is the least discoverable option. **Decision:** keep it and rely on the first-run overlay. Reason: it is the owner's choice.

## 3. Design tokens

All tokens are CSS custom properties with the `--l-` prefix, as in the Lab's `vars()` function. They are defined in `tokens.css` for light and dark. No component uses a literal color.

### 3.1 Colors, dark

| Token | Hex | Usage |
|---|---|---|
| `--l-bg` | `#0F1720` | Panel, window and page background |
| `--l-surface` | `#17232F` | Settings nav, setting rows, buttons, hover |
| `--l-line` | `#25394B` | Borders and dividers |
| `--l-text` | `#E6F1F8` | Primary text |
| `--l-muted` | `#8AA3B5` | Secondary text, hints, placeholders |
| `--l-accent` | `#5CC8E8` | Match highlight, focus, primary buttons, toggle on |
| `--l-ink` | `#062130` | Text on accent (KICKOFF name: onAccent) |

### 3.2 Colors, light

| Token | Hex | Usage |
|---|---|---|
| `--l-bg` | `#F4F8FB` | Panel, window and page background |
| `--l-surface` | `#E7EFF5` | Settings nav, setting rows, buttons, hover |
| `--l-line` | `#D0DEE8` | Borders and dividers |
| `--l-text` | `#0F2333` | Primary text |
| `--l-muted` | `#56707F` | Secondary text, hints, placeholders |
| `--l-accent` | `#0B7699` | Match highlight, focus, primary buttons, toggle on |
| `--l-ink` | `#F2FBFF` | Text on accent |

### 3.3 Semantic tokens

| Token | Dark | Light | Usage |
|---|---|---|---|
| `--l-sel` | accent at 17% over bg: `#1C3542` | accent at 13% over bg: `#D6E7EE` | Selected row, selected nav item |
| `--l-ok` | `#4CC38A` | `#1E7F50` | Running, key works, connected (from the Lab) |
| `--l-hover` | `--l-surface` | `--l-surface` | Row and button hover |
| `--l-muted-strong` | `#8AA3B5` | `#4B6474` | Muted text on surface or selected backgrounds |
| `--l-accent-text` | `#5CC8E8` | `#0C6A8A` | Accent-colored text on surface or selected backgrounds |
| `--l-ok-text` | `#4CC38A` | `#1C714C` | Ok text on surface |
| `--l-err` | `#F0766B` | `#B3382C` | Errors, Delete key text |
| `--l-warn` | `#E8B04A` | `#8A5A00` | Warning block bar and label |
| `--l-scrim` | `rgba(0,0,0,.5)` | `rgba(15,35,51,.35)` | Behind dialogs |

**Decision:** the last six tokens are not in the Lab. The three `-strong` and `-text` tokens are 85% of the base color mixed with 15% of `--l-text` in light theme, because the Lab's light pairs fall just under 4.5:1 (section 8.1). `err`, `warn` and `scrim` are new because the Lab has no error or warning state. Base hex values do not change.

### 3.4 Type scale

Fonts: `--l-ui` is IBM Plex Sans, `--l-mono` is IBM Plex Mono, both bundled as woff2 (no network fonts). Fallbacks: `system-ui`, "Segoe UI", `ui-monospace`, Consolas.

| Role | Size | Weight | Line height | Font |
|---|---|---|---|---|
| Input text | 17 px (`--l-fs` + 3) | 400 | 1.35 | Sans |
| Row title, body | 14 px (`--l-fs`) | 400 | 1.35 | Sans |
| Row subtitle | 11.8 px (.84em) | 400 | 1.35 | Sans, muted |
| Row meta, status | 11 px | 500 | 1.35 | Mono |
| Mode hint (`Command · Tab`) | 11 px | 500 | 1.35 | Mono |
| Action button | 12 px | 500 | 1.35 | Sans |
| Pill (provider, Fast/Think) | 11 px | 500 | 1.35 | Mono |
| Big math result | 30.8 px (2.2em) | 600 | 1.1 | Sans |
| Answer text | 14 px | 400 | 1.6, max 62ch | Sans |
| Toast | 13 px | 500 | 1.35 | Sans |
| Settings title | 20 px | 600 | 1.4 | Sans |
| Settings body, nav, description, dialog | 14, 13.5 (nav 500), 12.9 (muted), 13 | 400 | 1.4 to 1.5 | Sans |
| Group label, tag | 10.5 px | 500 | 1.4 | Mono, uppercase for groups |

**Decision:** file-type tile text is Mono 500, not 600. Reason: the Lab loads only Mono 400 and 500, so 600 would be faux bold.

### 3.5 Spacing, radius, heights, shadows

| Token | Value |
|---|---|
| Spacing scale | 2, 4, 6, 8, 10, 12, 14, 16, 20, 24 px. Settings main area uses 22 px side padding. |
| `--l-r` (panel) | 6 px |
| `--l-r2` (rows, buttons, fields) | 3 px |
| Pill, toggle / icon tile | 999 px / 4 px |
| `--l-row` | 44 px |
| `--l-input` | 56 px |
| Strip icon tile | 20 px (26 px in settings rows, 32 px in settings) |
| Toggle | 42 by 22 px, knob 13 px |
| Settings nav width | 200 px |

Solid material:

| Property | Dark | Light |
|---|---|---|
| `--l-panel` | `#0F1720` (= bg) | `#F4F8FB` (= bg) |
| `--l-border` | `1px solid #25394B` | `1px solid #D0DEE8` |
| `--l-shadow` | `0 22px 48px -18px rgba(0,0,0,.5)` | `0 22px 48px -18px rgba(30,30,50,.22)` |
| `--l-blur` | `none` | `none` |

The settings window uses `0 30px 60px -24px rgba(0,0,0,.5)` and a 1 px `--l-line` border. In light theme the border is only 1.29:1 against the page. That is fine for a decorative edge, because the shadow also defines it. Form fields use `--l-muted` borders instead (section 8.1).

## 4. Launcher layout

### 4.1 Sizes

| Part | Rule |
|---|---|
| Width | `min(700px, 100%)`, centered. **Decision:** on a narrow monitor keep a 16 px side margin. |
| Position | Centered horizontally, top edge at 28% of the height of the monitor under the mouse cursor |
| Input row | 56 px high, 16 px side padding, 10 px gap between icon, field and mode hint, 1 px bottom line |
| Results area | 6 px padding, 2 px gap between rows |
| Row | 44 px high, 10 px side padding, 10 px gap, 3 px radius |
| Group labels | Hidden in the strip layout |
| Footer | None |

### 4.2 Growth rules

| State | Height |
|---|---|
| Command, 5 rows | 297 px (56 + 1 + 6 + 5×44 + 4×2 + 6) |
| Fewer rows | Shrinks to fit; 0 rows shows one 44 px message |
| Find, selected row with action row | Adds about 42 px (4 + 28 + 8 padding, plus 2 gap): about 339 px |
| Query, empty or short answer | Fits the block, about 190 px |
| Query, long answer | Grows with the text up to 60% of monitor height, then the answer scrolls inside |

The cap is for the whole window. With the top edge at 28%, the bottom never passes 88%. **Decision:** no animation on growth; the native window resizes to the measured content height in whole pixels. Reason: nothing may animate on a keystroke.

### 4.3 Wireframes

Command mode, results shown, first row selected:

```
+----------------------------------------------------------------+
| (>)  Open an app or run a command            Command · Tab     |  56
+----------------------------------------------------------------+
|  [S] Spotify   Store app                                    ↵  |  44 selected
|  [S] Steam     Desktop app                                  30x|  44
|  [S] Slack     Desktop app                                  8x |  44
|  [D] Discord   Desktop app                                     |  44
|  [N] Notion    Desktop app                                     |  44
+----------------------------------------------------------------+
```

Find mode, selected row with action row:

```
+----------------------------------------------------------------+
| (/)  pfp                                        Find · Tab     |
+----------------------------------------------------------------+
|  [PNG] pfp_2026.png   C:\Users\Akshit\Pictures\Profile      ↵  |
|        [ Open ↵ ] [ Open file location Ctrl ↵ ] [ Copy path ]  |
|  [JPG] pfp_old.jpg    C:\Users\Akshit\Pictures\Profile         |
|  [DIR] Profile        C:\Users\Akshit\Pictures                 |
+----------------------------------------------------------------+
```

Query mode, streaming:

```
+----------------------------------------------------------------+
| (?)  how many tokens is 1000 words              Query · Tab    |
+----------------------------------------------------------------+
|  how many tokens is 1000 words                                 |
|  [Ollama · qwen3:4b] [Fast]                                    |
|  Roughly 1,300 to 1,400 tokens for English prose with most_    |
+----------------------------------------------------------------+
```

## 5. Components

State words: N normal, S selected, H hover, D disabled, F keyboard focus.

### 5.1 Input bar

Anatomy: mode icon (18 px, accent) at left, text field, mode hint at right (`Command · Tab`, mono 11, muted). The placeholder names the mode and is muted.

| State | Look |
|---|---|
| Empty | Placeholder in muted; 2 px accent caret |
| Typing | Text in `--l-text`, truncates with an ellipsis |
| Focus | Caret and accent icon. No outline ring (the field fills the row) |

### 5.2 Result row

Anatomy: tile (20 px), title, subtitle, meta. In the strip, title and subtitle sit on one line, 10 px apart, the subtitle baseline-aligned.

| State | Look |
|---|---|
| N | Transparent, 1 px transparent border |
| S | `--l-sel` fill. Meta shows `↵` in `--l-accent-text`. `aria-selected="true"` |
| H | `--l-hover` fill. Hover never moves the selection; a click does |
| D | 45% opacity, no hover, not selectable. Used for open location on Store apps |
| Armed (guarded) | Selected, and the subtitle reads "Press Enter again within 3 seconds to restart" |

**Decision:** hover uses the surface color. Reason: the Lab shows only a pointer cursor, and two accent fills would look like two selections.

### 5.3 App icon tile

20 px square, 4 px radius. Real app icons (the 32 and 64 px PNGs extracted by the app index) are shown when cached. The fallback is a letter tile: first letter, Sans 600 10.5 px, white on `hsl(h 48% 38%)`, with `h` chosen from the app name. Command items use the `cmd` tile: surface fill, accent letter, 1 px line inset. Math results use the `ans` tile: accent fill, ink text, "=".

**Decision:** the letter tile lightness is 38%, not the Lab's 46%. Reason: white on 46% reaches only 2.35:1 on yellow-green hues; 38% reaches at least 3.4:1.

### 5.4 File-type tile

Surface fill, 1 px inset line, extension in Mono 500 8.5 px, muted (for example `PNG`, `PDF`, `MD`). Folders show a 14 px folder outline icon in accent instead of text.

### 5.5 Group label

Mono 10.5 px, uppercase, 0.08em tracking, muted, padding 8 10 4. Hidden in the strip; used in Settings ("Modes", "Local").

### 5.6 Action row

Sits under the selected Find row, left padding 40 px, 6 px gap, 4 px top and 8 px bottom padding.

| Button | Look |
|---|---|
| Primary (Open) | Accent fill, ink text, no border, 12 px Sans 500, padding 5 10, 3 px radius, key cap at 75% opacity |
| Secondary | Surface fill, `--l-text`, 1 px line border |
| H | Border becomes `--l-muted` |
| F | 2 px accent outline, 2 px offset |
| D | 45% opacity |

Buttons are clickable but outside the tab order in the launcher. Keys do the work.

### 5.7 Query answer block

Padding 14 16 16, 10 px gap: question (600), chips row, answer.

- **Provider chip:** accent pill, ink text, Mono 11 (`Ollama · qwen3:4b`). Click or `Ctrl+P` cycles provider.
- **Fast/Think pill:** line border and muted text when Fast; accent fill when Think. Hidden if the provider has no reasoning control. `Ctrl+T` toggles.
- **Answer:** Sans 14, line height 1.6, 62 characters wide. Markdown renders with no raw HTML, no remote images, and links open only on click.
- **Streaming cursor:** 7 px by 1 em, accent, 1 s blink (solid with reduced motion). It disappears at the end; a stopped stream shows "Stopped." in Mono 11 muted.

### 5.8 Query empty state

For Query mode with no provider switched on. Same block as 5.7 with no chips.

| Part | Spec |
|---|---|
| Title | "Connect a model to ask questions", 14 px, 600, `--l-text` |
| Body | "Local models are free. Command and Find work without AI." 13 px, muted |
| Button | Primary action button "AI providers" with `↵` key cap. `Enter` also triggers it |
| Tile | 20 px `cmd` tile with `?` |

It is a calm state, not an error. No red, no warning icon.

### 5.9 Big math result

A block with `--l-sel` fill, padding 12 14: the result in 30.8 px Sans 600 (`5.512 lb`) and a 11.8 px muted working line (`2.5 kg × 2.20462 · ↵ copies`) that becomes "Copied" for 500 ms after `Enter`.

### 5.10 Toast

Inverted pill (`--l-text` fill, `--l-bg` text), 13 px 500, padding 8 14, bottom center. Fades in over 180 ms, stays 1.8 s, never takes focus, `role="status"`.

### 5.11 Settings window shell

1 px line border, 6 px radius, up to 1020 px wide. Title bar (9 14 padding, 12.5 px muted) with brand mark and "Settings". A 200 px surface nav with a right line, and a main area (20 22 24 padding, 12 px gap).

### 5.12 Nav item

13.5 px Sans 500, padding 8 12, gap 10, 3 px radius, muted text.

| State | Look |
|---|---|
| N | Muted text |
| H | `--l-text` |
| Current | `--l-sel` fill, `--l-text`, 3 px accent bar at left (28% inset top and bottom). `aria-current="page"` |
| F | 2 px accent outline |
| Count | Mono 11 muted at right (`9/10`) |

### 5.13 Settings row

Surface card, 1 px line, 3 px radius. 12 14 padding, 12 px gap: 32 px tile, text (name 600, description 12.9 px muted), status, Options button, toggle. Expanded options are a two-column list (name muted, value Mono) with 58 px left padding.

### 5.14 Toggle switch

Pill, 42 by 22, 1.5 px border, `role="switch"`.

| State | Look |
|---|---|
| Off | Transparent, `--l-muted` border, muted knob at left |
| On | Accent fill and border, ink knob moved 20 px |
| D | 45% opacity, `not-allowed` cursor. Command mode is on and locked |
| F | 2 px accent outline, 2 px offset |

The knob moves in 150 ms; none with reduced motion.

### 5.15 Tag pills and status text

Tag: Mono 10.5, 1 px line border, 999 px radius, padding 1 7, muted. Local tags use accent text and an accent border. Version tags read `v0.0.1`.

| Status | Style | Example |
|---|---|---|
| ok | `--l-ok-text` | `Running · qwen3:4b`, `Key works`, `Connected as Akshit` |
| neutral | `--l-muted-strong` | `Not running`, `Not set up`, `No key` |
| warn | `--l-warn` | `Host changed. Enter the key again.` |
| error | `--l-err` | `Key rejected`, `Not reachable` |

Status text is Mono 11.5 and always words, never color alone.

### 5.16 Options expander

Button "Options", Sans 12, 1 px line border, padding 4 9, `aria-expanded`. It opens in place with no animation and reads "Hide" while open.

### 5.17 Add key dialog

Modal inside the Settings window. Scrim `--l-scrim`. Dialog 440 px wide, `--l-bg`, 1 px line border, 6 px radius, 20 px padding, 14 px gap.

| Part | Spec |
|---|---|
| Title | "Add API key for OpenRouter", 16 px 600 |
| Host line | Label "Sent only to", then host in Mono 13 (`openrouter.ai`) |
| Key field | Password type, 36 px high, Mono 13, 1 px `--l-muted` border, 3 px radius. No show toggle. Initial focus |
| Warning block | Surface fill, 1 px line border, 3 px `--l-warn` bar at left, padding 12 14. Label "Read before saving" in `--l-warn`, Mono 11 uppercase. Body 13 px `--l-text` |
| Checkbox | 18 px box, 1.5 px muted border, 3 px radius, accent fill with ink tick when checked. Label 13 px |
| Footer | Right aligned: Cancel (secondary), Save key (primary), 32 px high |

Exact warning body, shown every time a key is added or replaced:

> Your key is stored in Windows Credential Manager and is only sent to <host>. No storage is completely safe: malware on your account, a compromised extension or a breach at the provider could still expose it. Add it at your own risk. Before saving, set a spending or usage limit in your provider's dashboard, and use a key created only for Flick so you can revoke it on its own.

Exact checkbox label: "I understand the risk and have set a usage limit."

States: **Initial** has the box off, Save disabled (45% opacity) and the helper "Tick the box to enable Save." (muted 12 px). Save stays disabled while the key is empty. **Ready** is key not empty and box ticked. **Saving** reads "Saving…" with fields disabled. **Error** shows "Keys are only sent over HTTPS." in `--l-err` under the field and keeps Save disabled.

`role="dialog"`, `aria-modal`, focus trapped, `Esc` closes, focus returns to the Add API key button. Local providers without a key never open this dialog.

### 5.18 Masked key display

A row inside the provider's Options. Pattern: `•••• 4f2a · openrouter.ai · last used 2 h ago`. The dots are plain text, never a real value. The UI only receives `{present, last4, host, added_at}`.

States: **Present** shows Test key, Replace and Delete key (error-colored text). **None** shows `No key` and a primary "Add API key". **Testing** shows "Testing…" and disables the buttons. **Works** or **rejected** shows `Key works` (ok) or `Key rejected` (error). **Host changed** removes the key and shows, in warn, "Host changed. Enter the key again." **Confirm delete** is inline: "Remove this key from Windows Credential Manager?" with Remove key and Cancel.

There is no Reveal and no Copy. "Last used" shows "never" before first use.

### 5.19 Explain preview and overlay

The Explain preview shows "Text from VS Code · 14 lines", up to 8 lines in a Mono surface box, and "Will be sent to Ollama · qwen3:4b (this computer)" in muted text. A blocked secret shows the message in `--l-err` and no send button. The prefixes overlay is a surface card with key caps (Mono 12, 1 px border, 2 px bottom border).

## 6. Settings window layout

Nav items: General, Features (`9/10`), AI providers, Decision engine, Integrations, About.

```
+------------------------------------------------------------------------+
| (i) Flick   Settings                                       -    []   x |
+----------------+-------------------------------------------------------+
| General        |  Features                                             |
| Features  9/10 |  Each feature is its own module. Switched off, it     |
| AI providers   |  loads nothing: no index, no watcher, no network.     |
| Decision engine|                                                       |
| Integrations   |  MODES                                                |
| About          |  [>] Command mode  Always on                  [ON-L]  |
|                |  [/] Find mode          [Options]             [ON ]   |
|                |  [?] Query mode                               [OFF]   |
|                |  FEATURES                                             |
|                |  [↑] Frecency ranking v0.0.1  [Options]       [ON ]   |
|                |      Ranks by how often and how recently...           |
|                |      Half-life         7 days                         |
|                |  [=] Inline math and units v0.0.1 [Options]   [ON ]   |
+----------------+-------------------------------------------------------+
```

- **AI providers:** groups "Local", "Free gateways", "Your own key". None is preselected or marked recommended.
- **Decision engine:** the Built-in parser row is locked on (a parser, not a model). Laya, Kev, Your AI provider and Jev are off. A dashed note explains `pip install laya`, then `laya-serve`.
- **Integrations:** Spotify, YouTube, Obsidian, Notion and webhook cards. Secret fields use the masked key display.
- Narrow window (700 px or less): nav becomes a top row, counts and status hide, options lose their left indent.

## 7. Motion

| Event | Spec |
|---|---|
| Launcher show | 100 ms fade, scale 0.98 to 1, ease-out `cubic-bezier(.2,0,0,1)` (**Decision:** no overshoot) |
| Launcher hide | Instant (**Decision:** to feel fast) |
| Keystroke, selection, mode switch, window growth | No animation |
| Toggle knob / toast / cursor | 150 ms / 180 ms with 8 px rise / 1 s blink |

With `prefers-reduced-motion: reduce`, show is instant, the cursor is solid, and the toggle and toast are instant.

## 8. Accessibility

### 8.1 Contrast

Ratios are computed WCAG values, rounded. Target is 4.5:1 for text under 18 px and 3:1 for large text and UI boundaries.

| Pair | Dark | Light | Result |
|---|---|---|---|
| `text` on `bg` | 15.7 | 15.0 | Pass |
| `muted` on `bg` | 6.9 | 4.9 | Pass |
| `accent` on `bg` | 9.4 | 4.8 | Pass |
| `ink` on `accent` | 8.6 | 4.9 | Pass |
| `muted` on `surface` | 6.1 | 4.49 | Light: use `muted-strong` (5.4) |
| `muted` on `sel` | 4.9 | 4.1 | Light: use `muted-strong` (4.9) |
| `accent` on `sel` | 6.7 | 4.1 | Light: use `accent-text` (4.8) |
| `ok` on `surface` | 7.2 | 4.3 | Light: use `ok-text` (5.1) |
| `err` on `surface` | 5.7 | 5.1 | Pass |
| `warn` on `surface` | 8.2 | 5.1 | Pass |
| `text` on `sel` | 11.2 | 12.6 | Pass |
| Toast: `bg` on `text` | 15.7 | 15.0 | Pass |
| Toggle off border `muted` on `surface` | 6.1 | 4.5 | Pass (3:1 needed) |
| Focus ring `accent` on `surface` | 8.3 | 4.4 | Pass (3:1 needed) |
| `line` on `bg` | 1.5 | 1.3 | Decorative only |

Rules: in light theme never put `--l-muted` text on selected or surface backgrounds; use the `-strong` and `-text` tokens. Fields and checkboxes use `--l-muted` borders. Disabled controls are exempt from contrast, but their label must explain why.

### 8.2 Keyboard map

| Key | Action |
|---|---|
| Hotkey (`Alt+Space`) | Show or hide |
| `Up` / `Down` | Move selection |
| `Enter` | Primary action (ask in Query) |
| `Ctrl+Enter` | Open location (Find, Command) |
| `Ctrl+Shift+Enter` | Run as admin (Command) |
| `Ctrl+C` | Copy path (Find) or answer (Query), only with no text selected |
| `Ctrl+Shift+T` | Open folder in Terminal (Find) |
| `Ctrl+T` | Fast/Think (Query) |
| `Ctrl+P` | Cycle provider (Query) |
| `Tab` / `Shift+Tab` | Next / previous mode |
| `>` `/` `?` first | Switch mode |
| `Esc` | Cancel stream, clear input, then hide |
| `Backspace` | Kill process after arrow selection (ports) |
| Settings: `Tab`, `Space`, `Enter` | Move focus, toggle, press |

### 8.3 Screen reader and focus

- Input: `role="combobox"` with `aria-controls` and `aria-activedescendant`. Results: `role="listbox"`, rows `role="option"` with `aria-selected`.
- A polite live region announces "5 results. pfp_2026.png selected. Enter to open." and guarded prompts.
- Answers use `aria-busy` while streaming and announce "Answer ready" at the end. **Decision:** tokens are not announced one by one.
- Toasts: `role="status"`. Dialogs: `role="dialog"` with `aria-labelledby`. Toggles: `role="switch"`. Nav: `aria-current`. Options: `aria-expanded`.
- Focus ring: 2 px `--l-accent`, 2 px offset. In the launcher, focus stays in the input, shown by the caret. Status is always text.

## 9. Content and voice

Rules:

1. Plain words, short sentences, present tense. Say what happened, then what to do.
2. No blame, no "oops", no exclamation marks, no emojis.
3. Name the thing: the app, provider or file, not "an error".
4. Say where data goes. Be honest about limits.
5. Guarded prompts say the key and the time.

| Situation | Copy |
|---|---|
| Launch failed | "Could not start Spotify. Windows said: <message>. Press Enter to try again." |
| No results | "Nothing matches "xyz". Press Tab to look in files instead." |
| Query empty state | "Connect a model to ask questions" / "Local models are free. Command and Find work without AI." |
| Guarded command | "Press Enter again within 3 seconds to restart." |
| Kill process | "End node.exe (PID 18244)? Enter to end, Esc to cancel." |
| Provider rejected | "OpenRouter rejected the key (401). Check it in Settings." |
| Secret blocked | "This text looks like a secret (an API key pattern). Flick did not send it." |
| Delete key | "Remove this key from Windows Credential Manager?" |

Provider status labels: `Running · qwen3:4b`, `Not running`, `No key`, `Key rejected`, `Key works`, `Not reachable`, `Not set up`, `Connected as <name>`, `Testing…`.

## 10. Iconography

No icon font and no brand logos. Flick never draws or ships a third-party logo.

- **Letter tiles** (5.3, 5.4) for commands, web shortcuts, providers, integrations and features.
- **Real app icons** are the one exception: Windows supplies them and Flick shows them as is (ADR-002). The letter tile is the fallback. **Decision:** this reconciles "no brand logos" with ADR-002.
- **Line icons** only for modes (bolt, magnifier, speech bubble) and folders: 2 px stroke, `currentColor`, 18 px and 14 px.

## 11. Dark and light behavior

- The theme follows Windows. Settings, General offers System, Dark and Light, applied at once to both windows.
- The accent differs by theme. Use the token, never a hex.
- The WebView background is `--l-bg` at creation, so the prewarmed window never flashes white.

## 12. Do and don't

| Do | Don't |
|---|---|
| Use tokens for every color, radius, height and font | Write a literal color in a component |
| Show the guarded prompt in the row | Use a modal for Restart or Kill |
| Show the text before Explain this sends it | Send selected text silently |
| Show a masked key and a Delete key button | Show, copy or return a stored key |
| Show the warning and checkbox every time a key is added | Skip the warning for a key the user already has elsewhere |

## 13. Handoff notes for implementers

1. `tokens.css` is the only file with literal colors. It defines `:root` (light), `@media (prefers-color-scheme: dark)` and `[data-theme="light"]` and `[data-theme="dark"]` overrides.
2. Property names match the Lab's `vars()` output exactly: `--l-bg`, `--l-surface`, `--l-line`, `--l-text`, `--l-muted`, `--l-accent`, `--l-ink`, `--l-sel`, `--l-panel`, `--l-border`, `--l-shadow`, `--l-blur`, `--l-r`, `--l-r2`, `--l-row`, `--l-input`, `--l-fs`, `--l-ui`, `--l-mono`, `--l-ok`. New: `--l-hover`, `--l-muted-strong`, `--l-accent-text`, `--l-ok-text`, `--l-err`, `--l-warn`, `--l-scrim`.
3. Values: `--l-r: 6px`, `--l-r2: 3px`, `--l-row: 44px`, `--l-input: 56px`, `--l-fs: 14px`. `KICKOFF.md` calls ink `onAccent`; keep `--l-ink` in CSS and map the name in the token source.
4. Keep the Lab class names (`ln-*`, `sw-*`, `tg`) as the starting vocabulary. New: `sw-dialog`, `sw-warn`, `sw-key`.
5. The Lab loads fonts from Google Fonts. The app must bundle woff2: Plex Sans 400, 500, 600 and Plex Mono 400, 500.
6. Keyboard state is a pure reducer with Vitest tests, including Esc ladder, prefix stripping, armed rows and the Backspace rule.
7. Dialog logic: Save enabled only when `key.length > 0 && acknowledged`. The key is sent once through a dedicated command and the input is cleared. The UI never stores, logs or echoes it.
8. The WebView makes no network calls. Content-Security-Policy: `default-src 'self'; connect-src ipc:`. Render model Markdown with raw HTML off.
9. The brand name comes only from `src/brand.ts`.
