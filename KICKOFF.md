You are the CONTROLLER for this build. If .agent/HANDOFF.md exists, this is a resumed session: follow "Resuming" in section 10 first. Otherwise read this whole file, write the plan, then run the build to the end without asking me anything (section 9). Keep .agent/HANDOFF.md current the whole time (section 10). You plan, assign, track, commit and decide. You do not write product code yourself; every source change goes through an executor subagent.

# 0. Inputs

DESIGN_CONFIG (exported from the Launcher Design Lab; if I paste a newer export between the markers, use that):

<<<DESIGN_CONFIG
{
  "name": "flick",
  "subdomain": "flick.akshitbansal.dev",
  "palette": "glacier",
  "material": "solid",
  "layout": "strip",
  "modeSwitcher": "hints",
  "type": { "set": "technical", "ui": "IBM Plex Sans", "mono": "IBM Plex Mono" },
  "corners": "sharp",
  "density": "comfy",
  "tokens": {
    "dark":  { "bg": "#0F1720", "surface": "#17232F", "line": "#25394B", "text": "#E6F1F8", "muted": "#8AA3B5", "accent": "#5CC8E8", "onAccent": "#062130" },
    "light": { "bg": "#F4F8FB", "surface": "#E7EFF5", "line": "#D0DEE8", "text": "#0F2333", "muted": "#56707F", "accent": "#0B7699", "onAccent": "#F2FBFF" }
  },
  "radiusPx": [6, 3],
  "rowHeightPx": 44
}
DESIGN_CONFIG>>>

APP_NAME = DESIGN_CONFIG.name. Bundle identifier = dev.akshitbansal.<APP_NAME>. The name must be renameable later by editing one constant in Rust (crates/launcher-core/src/brand.rs), one in TS (apps/desktop/src/brand.ts), one in the web app (apps/web/src/brand.ts) and tauri.conf.json. Nowhere else may hardcode it.

SPEC DOCS in docs/spec/: 01-PRD.md (requirements with FR/NFR IDs), 02-TRD.md (technical requirements), 03-AFD.md (app flows and screen states), 04-UIUX-Design-Brief.md (tokens, components, states), 05-BSD.md (traits, Tauri command API, storage schema), 06-Implementation-Plan.md (waves and cards). Put the relevant spec sections in each card's `reads`, and quote FR/NFR IDs in the card's `accept`. If a spec doc conflicts with this file, this file wins; log the conflict in .agent/LOG.md and fix the spec doc in a docs card.

REVIEW_MODE = per-wave   (per-wave | per-task)

Machine: Windows 11, 24 GB RAM that is often near full. Write agent commands for the shell Claude Code uses on this machine (usually Git Bash); scripts meant for me (scripts/*.ps1) are PowerShell and are run with `pwsh -File scripts/<name>.ps1`. Never run more than one heavy build at a time (see 6.6).

# 1. Product

A free, open-source, keyboard-first Windows launcher with three modes in one bar.

Flick is a wrapper. It ships no models, no keys, no account and no default provider, and nothing AI-related is on out of the box. The user decides what to connect: local models (Ollama, LM Studio, llama.cpp server), open-source decision models (Laya, Kev), free gateways (OmniRoute, OpenRouter free models) or their own paid keys (Anthropic, OpenAI, Jev). Every feature must work with no paid account, and Command and Find work fully with no AI at all. No telemetry, no sign-in, MIT license.

The build has two milestones in this one run. Tag each one.

## 1.1 Milestone v0.0.1: core

1. Global hotkey Alt+Space (configurable) toggles a frameless, always-on-top launcher centered at 28% from the top of the monitor under the mouse cursor. The window is created once at startup, kept hidden, never destroyed. It hides on Esc, on focus loss, and after a successful launch.
2. Command mode. Fuzzy-match installed apps exactly as Start's "All apps" lists them, Store apps included (Spotify from the Store must work). App entries only: never folders, documents, uninstallers or help links. Enter launches, Ctrl+Shift+Enter runs as admin, Ctrl+Enter opens the location. Empty input shows the most-used apps.
3. Find mode. Fuzzy-match files and folders under the indexed roots. The selected result shows actions: Open (Enter), Open file location (Ctrl+Enter, explorer /select), Copy path (Ctrl+C).
4. Query mode. Streams an answer from the first enabled provider that responds, falling back down the user's list. A Fast/Think toggle in the bar controls reasoning on models that support it. No history by default. Esc cancels the stream. With no provider enabled it shows an empty state, "Connect a model to ask questions", with a button to Settings > AI providers.
5. Mode switching: Tab and Shift+Tab cycle modes. Typing ">", "/" or "?" as the first character switches to Command, Find or Query and the character is removed.
6. Feature system (ADR-009) with PowerToys-style toggles, plus these features:
   - Frecency ranking
   - Inline math and units
   - System commands
   - Web shortcuts
   - Project launcher
7. Settings window, PowerToys-style (section 5.2).
8. Tray icon with Open, Settings, Quit. Single instance. Launch at startup toggle.
9. First run: pick the hotkey (detect a failed registration, for example when PowerToys Run owns Alt+Space, and ask for another); detect running local model servers and offer them (nothing is switched on unless the user picks it); a one-time overlay teaching Tab and the > / ? prefixes (the hint-only switcher hides them otherwise).

## 1.2 Milestone v0.1.0: integrations

10. Natural-language finder (ADR-005)
11. Ports and processes
12. Media controls with Spotify and YouTube connections (ADR-010)
13. Explain this
14. Quick capture to Obsidian, Markdown file, Notion and webhooks (ADR-010)

## 1.3 Feature details

- Frecency ranking: exponential decay with a 7-day half-life, plus prefix learning (remember which item was chosen for a typed prefix). Options: half-life, learning on or off, clear history. Off means pure match order.
- Inline math and units: as-you-type results in Command mode for arithmetic, units, currency and time zones ("2.5 kg to lb", "250 usd in inr", "5pm ist in pst"). Use the fend-core crate for math and units and chrono-tz for zones. Currency uses the ECB daily reference rates XML (https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml), fetched at most once a day off the keystroke path and cached; offline uses the last cache and says how old it is. Enter copies the result.
- System commands: Lock, Sleep, Hibernate, Restart, Shut down, Sign out, Empty Recycle Bin, Restart Explorer, Wi-Fi on/off, Bluetooth on/off (Windows.Devices.Radios). Restart, Shut down, Sign out and Empty Recycle Bin need Enter twice within 3 seconds.
- Web shortcuts: keyword plus query opens a URL template with {q}. Defaults: g (Google), yt (YouTube), gh (GitHub), amz (amazon.in), maps (Google Maps), wiki (Wikipedia). Users add, edit, remove and reorder them in settings.
- Project launcher: folders containing .git, package.json, Cargo.toml, pyproject.toml or go.mod under dev roots (default D:\dev and %USERPROFILE%\source\repos when they exist), plus recent VS Code workspaces (read them from VS Code's own state store; verify the current location on this machine before coding). Keywords: "code <name>" opens the configured editor (VS Code by default; Cursor, Zed and others selectable), "term <name>" opens Windows Terminal there, a bare name opens the folder.
- Natural-language finder: see ADR-005.
- Ports and processes: "port 3000" lists the processes holding that TCP or UDP port (GetExtendedTcpTable and GetExtendedUdpTable through the windows crate, process details through sysinfo): name, PID, memory, uptime, command line. "proc <name>" searches processes. Backspace kills after a confirm.
- Media controls: see ADR-010.
- Explain this: a second hotkey (Alt+Shift+Space, configurable) grabs the selected text in any app, through UI Automation TextPattern first and a simulated Ctrl+C with clipboard save and restore as the fallback, then opens Query mode with the text attached. The text is shown before anything is sent. Text that looks like a secret (key patterns, long high-entropy tokens) is blocked from sending.
- Quick capture: see ADR-010.

# 2. Locked decisions

Do not revisit these. Write each as an ADR in docs/adr/ (Context, Decision, Alternatives, Consequences).

ADR-001 Shell and UI. Tauri 2 with all logic in Rust crates; UI in React 19 + TypeScript + Vite rendered by WebView2. Pure-Rust UI (egui, iced, Slint) was rejected: far slower to reach the visual quality of the design config. WebView2 startup cost does not matter because the window is prewarmed and only shown and hidden.

ADR-002 App index. Enumerate the Shell AppsFolder (FOLDERID_AppsFolder) with the `windows` crate. It is exactly Start's All apps list: classic shortcuts and packaged apps. Launch with ShellExecuteExW on `shell:AppsFolder\<parsing name>`; admin launch uses the "runas" verb. Filter out entries whose name matches (?i)\b(uninstall|readme|help|documentation|website|release notes|license|manual)\b and entries whose target resolves to a folder or a non-executable document. Extract icons to PNG (32 and 64 px). Cache the list and icons in %LOCALAPPDATA%\<APP_NAME>\cache so a cold start never waits for enumeration; refresh in the background every 10 minutes and on demand.

ADR-003 File index. Own index: jwalk for the initial parallel scan, notify for live changes, a compact in-memory list, and a persisted snapshot for instant cold start. Default roots: Desktop, Documents, Downloads, Pictures, Videos, Music, plus user roots. Skip node_modules, .git, target, dist, build, .next, AppData, $Recycle.Bin, hidden and system entries.

ADR-004 Matching. nucleo-matcher in both modes. Final rank = match score, plus frecency and prefix learning when that feature is on. Matching never calls the network.

ADR-005 Natural-language finder: parser first, decision engine second. Never on the keystroke path; it runs on Enter in Find mode when the input has 3+ words or a natural-language marker (my, from, in, the, that, last, yesterday).
- Stage 1, always on, offline: a built-in parser strips filler ("open", "my", "the"), maps location phrases to roots ("from pictures" to the Pictures root, "in downloads", "on desktop"), file-type words to extensions ("photo", "image", "pdf", "sheet"), and time words to modified-date filters ("yesterday", "last week"). It then runs the normal fuzzy match inside those filters. If the top result is clear (score above the threshold and at least 10% ahead of the second), open it.
- Stage 2, only when stage 1 is ambiguous: send the top 8 candidates (file name and parent folder only, never contents) to the first enabled, reachable decision engine as a single Choice question. Act if p >= 0.6; otherwise show the ranked choices. Cache input to chosen item.
- Engines, all off until the user enables one, in this order: Laya (open source, Apache-2.0, self-hosted with `laya-serve`), Kev (open source, self-hosted, Jev-compatible), the user's Query provider asked to pick a candidate number with a strict JSON reply, Jev (TypeSafe cloud, paid key), and Custom (any other server that speaks /v1/systemone, with its own URL and optional key). The user can enable several and reorder them. Laya, Kev and Jev all speak the same POST /v1/systemone protocol, so one client with a configurable base URL and optional bearer key serves all three. Before writing it, fetch https://docs.typesafe.ai/api.md and the Laya README at https://github.com/NandhaKishorM/laya and implement the documented schema; do not guess field names. Laya is a Python service, so the app never bundles it: settings explain how to run it (pip install laya, laya-serve) and probe the URL. Keep the candidate count small, because Laya's accuracy drops with many options and is weak without fine-tuning; that is why stage 1 exists.
- With every engine off (the default), the parser and the plain choices list still work.
- Never auto-open executables or scripts (.exe, .bat, .cmd, .ps1, .lnk, .msi and similar) from a natural-language request; they always go to the choices list for a manual Enter.

ADR-006 Providers, FOSS-first. One trait, roughly `Provider::stream(Request) -> impl Stream<Item = Result<Delta>>`, two wire formats: OpenAI Chat Completions and Anthropic Messages.
- Presets, in this order: Ollama (http://localhost:11434/v1), LM Studio (http://localhost:1234/v1), llama.cpp server (http://localhost:8080/v1), OmniRoute (http://localhost:20128/v1), OpenRouter (https://openrouter.ai/api/v1), Anthropic, OpenAI, Custom (any OpenAI-compatible base URL, model, optional key). All but Anthropic use the OpenAI format.
- On first run and on demand, probe the three local ports with GET /v1/models (200 ms timeout) and list the models found. Detected servers are offered, never switched on automatically. No provider is enabled by default and none is marked as recommended.
- Query mode uses the first enabled provider that responds and falls back down the user's order on connection errors, 429 and 5xx.
- Fast/Think maps to each provider's own reasoning control where it has one and is hidden where it has none. For Anthropic claude-haiku-5-5: never send temperature, top_p, top_k or an assistant prefill (Haiku 5.5 rejects them); fetch the current Messages API docs on docs.claude.com for the effort and thinking parameters before coding.
- Stream deltas to the UI through tauri::ipc::Channel. Esc aborts the request. System prompt: answer directly and briefly, markdown allowed, max_tokens 1024.

ADR-007 Storage and secrets. API keys and OAuth tokens live in Windows Credential Manager through the `keyring` crate and follow ADR-011. Never in config files, logs, panics, test snapshots or requests to decision engines. Config: TOML at %APPDATA%\<APP_NAME>\config.toml. Frecency, prefix learning and caches: SQLite via rusqlite with the bundled feature.

ADR-008 Repo and web. pnpm workspace plus Cargo workspace in one repo. Landing and docs site: current stable Next.js (App Router) + Tailwind + Framer Motion in apps/web, deployed on Vercel; I add the custom subdomain later.

ADR-009 Feature modules and toggles. Every feature in sections 1.1 item 6 and 1.2 is a module behind one trait in crates/features, roughly: `id()`, `title()`, `default_enabled()`, `settings_schema()`, `start(ctx)`, `stop()`, `query(&Input) -> Vec<Item>` (synchronous and cheap; anything slow is precomputed in `start`), `execute(&Item, Action)`. A registry owns them. Turning a feature off calls `stop()` and drops everything it holds: watchers, timers, caches, network clients. A disabled feature costs zero CPU, zero network and near-zero memory. Toggles apply instantly, no restart. Find mode and Query mode have toggles too; Command mode is always on. Config shape: `[features.<id>] enabled = true` plus that feature's options. Each feature's options page is generated from its `settings_schema()` so new features need no hand-written settings UI.

ADR-010 Integrations, FOSS-first.
- Media. Base layer, no account: Windows Global System Media Transport Controls (Windows.Media.Control through the windows crate) for play, pause, next, previous and now-playing on any player, including Spotify and YouTube in a browser. "play <song> on spotify" opens `spotify:search:<query>` in the desktop app; "play <x> on youtube" opens the YouTube search URL in the default browser. Optional Spotify connection: OAuth PKCE with the user's own Spotify developer app client ID (since February 2026, Spotify development-mode apps need the app owner on Premium and allow 5 users per app, so a shared client ID cannot work for a public app). Connecting Spotify is offered in Settings > Integrations as "unlock more Spotify features" and adds: play a specific song, album, artist or one of your playlists directly; add to queue; like or save the current track; switch the playback device; search your own playlists. Check the current Spotify Web API docs during the card for which of these need the user to have Premium, and show that next to each action. Without a connection, the base layer above still works. Optional YouTube Data API key to open the first result instead of the search page.
- Quick capture. Targets: Obsidian vault (append to today's daily note or a chosen file by writing the Markdown file directly; no plugin), plain Markdown file, Notion (internal integration token plus database ID; map the parsed fields to properties), webhook (POST JSON to a URL). Parse "log 250 lunch" style input locally; ask the Query provider only when the format is unclear.

ADR-011 Secret security. Applies to every secret: provider keys, OpenRouter, TypeSafe, Notion, YouTube, Spotify tokens, webhook secrets.
1. Storage: Windows Credential Manager (DPAPI, per Windows user). Never in config.toml, SQLite, logs, crash output, exports or backups. Entry names include the provider id and its host, as defined in 05-BSD.md (for example `provider.openrouter@openrouter.ai`).
2. Write-only from the UI: the WebView hands a key over once through `set_secret`; Rust stores it. No command ever returns a secret. The UI only receives {present, last4, host, added_at, last_used}. There is no `get_secret`.
3. All network calls are made in Rust, never from the WebView. Tauri CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; script-src 'self'`; no remote scripts, frames or images. Tauri capabilities are minimal: no shell, fs, http or opener plugin exposed to the frontend beyond the exact commands it needs.
4. Model output is untrusted: render Markdown with raw HTML disabled and sanitized, no remote images, links open in the browser only on click. Model output can never trigger an action by itself.
5. In memory: `secrecy::SecretString` with `zeroize`; secrets are not Debug or Display printable and are dropped as soon as the request is built.
6. A key is bound to the host it was entered for and is only sent over HTTPS to that host (plain http only for localhost and 127.0.0.1). Changing a provider's base URL host deletes the stored key and asks for it again. Authenticated requests do not follow redirects.
7. A tracing redaction layer masks key-like patterns and every stored secret value; the panic hook redacts too.
8. Explain this and Quick capture block text that looks like a secret.
9. CI runs cargo-deny, cargo-audit and pnpm audit; lockfiles are committed; releases ship SHA-256 checksums.
10. Every stored key has Test, Replace and Delete buttons and shows when it was last used.
11. Residual risk is never zero (malware running as the same Windows user can read Credential Manager; a provider breach or a hostile extension can leak keys), so every "Add key" dialog shows this warning and Save stays disabled until the checkbox is ticked:

    "Your key is stored in Windows Credential Manager and is only sent to <host>. No storage is completely safe: malware on your account, a compromised extension or a breach at the provider could still expose it. Add it at your own risk. Before saving, set a spending or usage limit in your provider's dashboard, and use a key created only for Flick so you can revoke it on its own."
    Checkbox: "I understand the risk and have set a usage limit."

    Local providers without a key never show it.

# 3. Repo layout

Ownership boundaries for executors follow this tree.

```
<APP_NAME>/
  apps/
    desktop/
      src/                 React UI: launcher/, settings/, query/, tokens.css, brand.ts
      src-tauri/           Tauri shell: window, hotkeys, tray, commands (thin; calls crates)
    web/                   Next.js landing, /docs, /changelog
  crates/
    launcher-core/         brand.rs, types, Mode, prefix parsing, matcher, ranking, SQLite store
    index-apps/            AppsFolder enumeration, filters, launch, icons, cache
    index-files/           scan, watch, snapshot, query
    providers/             Provider trait, openai_compat.rs, anthropic.rs, presets, detection, keys
    decision/              /v1/systemone client (Laya, Kev, Jev), LLM picker, NL parser
    features/              Feature trait, registry, settings schema
      src/frecency/ math/ syscmd/ web/ projects/ nlfind/ ports/ media/ explain/ capture/
    win/                   Win32 and WinRT wrappers used by several crates (power, radios, media session, UI Automation, port tables); the only crate besides index-apps allowed `unsafe`
  docs/                    architecture.md, adr/, roadmap.md, followups.md, performance.md
  scripts/                 release.ps1, measure.ps1
  .github/workflows/       ci.yml, release.yml
  Cargo.toml  package.json  pnpm-workspace.yaml  rust-toolchain.toml  biome.json  cliff.toml
  README.md  CHANGELOG.md  CONTRIBUTING.md  SECURITY.md  LICENSE (MIT)
```

# 4. Performance budgets

These are acceptance criteria, measured, not hoped for. Record measured numbers in docs/performance.md.

- Hotkey to window visible: p95 <= 50 ms (log monotonic timestamps; scripts/measure.ps1 prints the p95 over 50 toggles).
- Command match in Rust for 2,000 apps with every v0.0.1 feature on: <= 2 ms per query (criterion bench). Keystroke to painted results p95 <= 16 ms.
- Each feature's `query()` <= 0.5 ms (criterion bench per feature).
- Find match over 300,000 synthetic paths: p95 <= 25 ms (criterion bench).
- Enter to launch call returned: <= 30 ms (spawn, never wait on the launched app).
- Cold start to tray ready using caches: <= 600 ms.
- Idle: RAM <= 120 MB total including WebView2, CPU near 0%. With every feature off, nothing polls.
- Query: each streamed token is painted when it arrives; no buffering.

# 5. UI spec

## 5.1 Launcher

- Turn DESIGN_CONFIG.tokens into CSS custom properties for dark and light. Follow the Windows theme by default; settings can force dark or light.
- Material solid: opaque token background, no window effect.
- Layout strip: a thin bar with up to 5 one-line results and no footer. It grows when needed: Find shows the action row under the selected result, Query grows to fit the streamed answer up to 60% of the monitor height and then scrolls inside.
- Mode switcher hints: no visible control; the right end of the input shows the mode name and "Tab". The placeholder names the mode.
- Corners and rows follow radiusPx and rowHeightPx. Type is IBM Plex Sans and IBM Plex Mono, bundled locally as woff2 (no network fonts in a desktop app).
- The Design Lab (docs/spec/design-lab.html, open it in a browser) is the visual reference for every state. Where 04-UIUX-Design-Brief.md adds tokens (hover, errors, warnings, stronger muted text, darker letter tiles) for contrast, use the brief's values.
- Window: 700 logical px wide, 16 px side margin on narrow monitors.
- Keyboard model: Up and Down move the selection; Enter primary action; Ctrl+Enter location; Ctrl+Shift+Enter admin; Ctrl+C copy path in Find; Tab and Shift+Tab mode; Ctrl+T Fast/Think and Ctrl+P next provider in Query; Esc cancels a running answer, then clears the input, then hides; prefixes > / ?. 03-AFD.md has the full map.
- Show animation: 100 ms fade and 0.98 to 1 scale; nothing animates on keystroke; respect reduced motion.
- Accessibility: listbox and option roles, visible focus, every control reachable by keyboard.
- The UI holds no business logic. Keyboard state lives in a pure reducer with Vitest tests.

## 5.2 Settings window, PowerToys-style

A separate window in the same tokens. Left navigation: General, Features, AI providers, Decision engine, Integrations, About.
- General: hotkeys, theme, launch at startup, indexed roots, clear history.
- Features: a Modes group (Command locked on, Find, Query) and a Features group. Each row: icon, name, version tag, one-line description, an Options button that expands the options generated from `settings_schema()`, and a toggle switch. The nav shows the count switched on (for example 9/10).
- AI providers: grouped Local, Free gateways, Your own key, none switched on by default. Each row: toggle, status (detected with model name, not running, no key), Options (base URL, model, fallback order) and, for keyed providers, the masked key (•••• last4 · host · last used) with Test, Replace and Delete. Adding or replacing a key opens the Add key dialog from ADR-011 with the warning and the required checkbox.
- Decision engine: Built-in parser (locked on), Laya, Kev, Your AI provider, Jev, Custom /v1/systemone endpoint, all reorderable, each with toggle, status from a probe, and Options (URL, key, candidate count, threshold), plus a short note on how to start laya-serve.
- Integrations: Spotify (connect with your own client ID, with the steps), YouTube (optional key), Obsidian (vault folder), Notion (token and database), webhooks.
- About: version, license, links to repo, docs, changelog.
Toggle switches are pill-shaped like Windows 11. The Design Lab's Settings section is the reference.

# 6. Team and protocol

## 6.1 Roles

- Controller (you, this session). Owns .agent/PLAN.md and .agent/LOG.md, writes task cards, dispatches, commits, decides. Never edits files under apps/ or crates/.
- Executors x3: subagent_type "executor". Code changes, commands, reading files, scaffolding. Never commit.
- Tester x1: subagent_type "tester". Runs checks on each finished task. Never edits source.
- Reviewer x1: subagent_type "reviewer". Reviews diffs. Read-only.

Model and effort come from each agent's frontmatter (executor and tester: haiku, low; reviewer: opus, high). Upgrade a single call by passing model: "sonnet" on the Agent tool call.

## 6.2 Model choice per task card

- haiku: reading files, running commands, scaffolding from a known template, single-file changes, config, docs.
- sonnet: any card that owns more than one source file, any change to a public API used by another crate, any `unsafe` or Win32/WinRT/COM code, async streaming code, and any retry after a haiku failure.
- Tester goes to sonnet only when diagnosing a failure requires reading more than 3 files.

## 6.3 Task cards

Keep every card in .agent/PLAN.md in this shape:

```
### T<wave>.<n> <title>
model: haiku|sonnet   deps: <ids or none>   status: todo|doing|testing|done|blocked
owns: <exact files this card may create or edit>
reads: <files it may read for context>
do: <3 to 8 concrete lines>
accept: <exact commands that must pass>
```

A card owns at most 6 source files (W0 scaffolding cards may own more). Two cards running at the same time never own the same file. Root manifests (root Cargo.toml, package.json, pnpm-workspace.yaml), lockfiles, the features registry file and the settings navigation file belong to at most one running card; other executors report what they need under NEEDS and you add it in a later card. Feature modules live in their own folders so up to three can be built in parallel.

## 6.4 The loop

1. Write the whole plan into .agent/PLAN.md (both milestones, waves and cards). Create .agent/ and gitignore it.
2. Dispatch up to 3 executor cards in ONE message so they run in parallel. Pass the full card text as the prompt.
3. As soon as an executor reports DONE, dispatch the tester on that card in the background while the other executors keep working. Keep all three executor slots busy with the next ready cards.
4. Tester PASS: stage only that card's owned files and commit following my global commit conventions in ~/.claude/CLAUDE.md. One commit per card. Executors never commit.
5. Review:
   - per-wave (default): when every card in the wave has passed, send the reviewer `git diff <wave-start-sha>..HEAD` plus the wave's cards.
   - per-task: send the reviewer each card's commit right after the tester passes it, in parallel with the tester on the next card.
6. Reviewer findings: BLOCKER becomes a fix card at the top of the next wave and runs before anything else. SHOULD-FIX items are batched into one card in the next wave. NITs go to docs/followups.md and are not fixed now.
7. Update PLAN.md statuses and add one line to LOG.md for every dispatch, result, commit and decision.

## 6.5 Failure ladder

Attempt 1 on the card's model. FAIL: attempt 2, same model, with the tester's log. FAIL: attempt 3 on sonnet, or split the card into smaller cards. FAIL again: mark the card blocked, keep building everything that does not depend on it, and list it in the final report. Never loop on the same failure more than three times.

## 6.6 Speed and resource rules

- Executors run only fast checks for their own package: `cargo check -p <crate>`, `cargo test -p <crate>`, `pnpm --filter <pkg> typecheck`, `pnpm --filter <pkg> test`.
- Only the tester runs heavy builds (`cargo build` of the app, `pnpm tauri build`, `pnpm --filter web build`), and only one heavy build runs at a time.
- One shared Cargo target directory. Do not create per-agent target dirs.
- Subagent reports stay within 15 lines. You do not read whole large files; use the reports, `git diff --stat` and targeted Grep.
- Live tests are `#[ignore]` and run only when their service is available: OLLAMA_URL, LAYA_URL or KEV_URL set and reachable, or ANTHROPIC_API_KEY, OPENROUTER_API_KEY, TYPESAFE_API_KEY, NOTION_TOKEN set. Never print a key. Everything else uses recorded fixtures (SSE streams, /v1/systemone responses, Notion responses).

# 7. Waves

Refine the cards in PLAN.md, but keep this order and these boundaries.

## Milestone v0.0.1

W0 Preflight and skeleton (haiku executors in parallel: Rust workspace, JS workspace and Tauri app, CI and repo files; then you commit).
- Check: rustup stable with the x86_64-pc-windows-msvc target, MSVC Build Tools (link.exe), WebView2 runtime, Node 20+, pnpm, git, gh, vercel, git-cliff (install with cargo install if missing). If MSVC Build Tools or WebView2 are missing, install them with winget; if that needs an admin prompt, follow section 9.
- Create the workspace from section 3: manifests, rust-toolchain.toml, biome.json, .editorconfig, .gitignore, LICENSE (MIT, Akshit Bansal); keep the CLAUDE.md, KICKOFF.md and .claude/ already in the folder and commit them; empty crates that compile; a Tauri app that opens a blank hidden window; a Next.js app that builds.
- ci.yml on windows-latest: cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test, cargo deny check, cargo audit, pnpm typecheck, pnpm lint, pnpm test, pnpm audit --prod, web build.
- cliff.toml that groups my commit prefixes (feat/, fix/, up/, rem/, sm/, add/, add-p/, hide/, init/) into changelog sections.
- git init, first commit.

W1 (three in parallel)
- A crates/launcher-core: brand.rs, types, Mode, prefix parsing, nucleo matcher wrapper, ranking hooks, SQLite store, unit tests, criterion bench. sonnet.
- B crates/index-apps: AppsFolder enumeration, filters, launch (normal, runas, reveal location), icons, cache, tests for the filter rules. sonnet.
- C apps/desktop/src-tauri shell: prewarmed hidden window, global shortcut toggle with timing log, hide on blur, tray, single instance, autostart. sonnet.

W2 (three in parallel)
- A crates/index-files: scan, skip rules, watcher, snapshot, query, criterion bench over 300,000 synthetic paths. sonnet.
- B crates/providers: trait, OpenAI-compatible client, Anthropic client, presets, local detection, fallback order, cancellation, SSE tests on recorded fixtures; secret store per ADR-011 (Credential Manager, SecretString, host binding, HTTPS-only, no redirects on authenticated requests) with tests that a key is never sent to another host and never appears in Debug output or logs. sonnet.
- C apps/desktop/src launcher UI: tokens.css from DESIGN_CONFIG, bundled fonts, strip layout with growth rules, hint switcher, three modes, results, Find actions, Query view with Fast/Think, keyboard reducer with Vitest tests. sonnet.

W3 (three in parallel)
- A crates/features: Feature trait, registry, settings schema, start/stop lifecycle tests (a disabled feature holds no tasks). sonnet.
- B crates/win: power and session (lock, sleep, hibernate, restart, shut down, sign out), recycle bin, Explorer restart, radios. sonnet.
- C Tauri commands and wiring: search, launch, open, reveal, copy path, ask_stream over Channel with cancel, feature toggle and options commands, write-only secret commands (set_secret, delete_secret, test_secret, list_secrets), strict CSP and minimal capabilities, tracing redaction layer and redacting panic hook. sonnet.

W4 (three in parallel; one feature folder each)
- A features/frecency and features/web. sonnet.
- B features/math (fend-core, chrono-tz, ECB rate cache) and features/syscmd with the double-Enter guard. sonnet.
- C features/projects (roots, markers, VS Code recents, editor choice). sonnet.

W5 (three in parallel)
- A Settings window: General, Features, AI providers (probe status, masked keys, Add key dialog with the ADR-011 warning and required checkbox), About; options pages generated from settings schemas; Query empty state. sonnet.
- B First run: hotkey capture and conflict detection, local model detection, the one-time prefixes overlay. sonnet.
- C Performance pass: benches and scripts/measure.ps1, fix anything over budget, docs/performance.md. sonnet.

W6 Docs, site, release v0.0.1 (three in parallel, then release)
- A Docs: README (what it is, FOSS-first, install, hotkeys, modes, features, providers including local setup, privacy, build from source), docs/architecture.md with a mermaid diagram, ADR-001 to ADR-010, CONTRIBUTING, SECURITY, roadmap.md. haiku, one file per card.
- B apps/web: landing page whose hero is a live launcher demo built from the same tokens, /docs from the markdown docs, /changelog from CHANGELOG.md, download button pointing at the latest GitHub release. sonnet.
- C Release tooling: scripts/release.ps1 <version> (sets the version in the Cargo workspace, tauri.conf.json and every package.json, runs git-cliff, commits, tags); release.yml on tag v* using tauri-action to build the NSIS installer on windows-latest and attach it with a SHA-256 checksum. The installer is unsigned; note the SmartScreen prompt in the README. sonnet.
- Then: reviewer reviews the whole repo against this file and CLAUDE.md, fix BLOCKERs, run release.ps1 0.0.1, push, deploy the site (section 9).

## Milestone v0.1.0

W7 (three in parallel)
- A crates/decision: NL parser (stage 1) with a table-driven test suite of at least 40 phrases; /v1/systemone client for Laya, Kev and Jev; LLM picker with strict JSON parsing; recorded-response tests. sonnet.
- B crates/win additions: media session (GSMTC), port tables, UI Automation selected text with Ctrl+C fallback and clipboard restore. sonnet.
- C Settings: Decision engine page and Integrations page. sonnet.

W8 (three in parallel; one feature folder each)
- A features/nlfind wired to the decision crate and Find mode. sonnet.
- B features/ports (with "proc") and features/media (base controls, spotify: and YouTube search, optional Spotify PKCE and YouTube key). sonnet.
- C features/explain (second hotkey, preview, secret blocking) and features/capture (Obsidian, Markdown, Notion, webhook). sonnet.

W9 Release v0.1.0
- Per-feature benches against the 0.5 ms budget, docs for every new feature and integration (including how to run laya-serve and how to make a Spotify developer app), landing page feature section, full-repo review, fix BLOCKERs, release.ps1 0.1.0, push, deploy.

# 8. Git, GitHub, Vercel

- Branch main. Small commits, one per card, using my global conventions. Semantic versioning: 0.0.1, then 0.1.0. CHANGELOG.md generated by git-cliff only.
- GitHub: after W0, if `gh auth status` succeeds, create the private repo akshit-bansal11/<APP_NAME> and push. Push after every wave. I make it public myself.
- Vercel: after W6-B, if `vercel whoami` succeeds, link apps/web as its own project with root directory apps/web and deploy to production. I add <APP_NAME>.akshitbansal.dev later.

# 9. Autonomy

Decide for yourself. Do not ask me questions, do not ask for confirmation, and do not wait for approval at any point. When something is unclear, pick the most reasonable option, write the decision and the reason in .agent/LOG.md, and keep going.

- GitHub: if `gh auth status` succeeds, create the private repo akshit-bansal11/<APP_NAME> after W0 and push after every wave. If gh is not logged in, keep committing locally and list "run gh auth login" under Needs Akshit in the handoff.
- Vercel: if `vercel whoami` succeeds, link and deploy apps/web after W6-B and after W9. If not, list "run vercel login" under Needs Akshit.
- Missing tools: install them yourself (rustup, cargo install, pnpm, winget for MSVC Build Tools and the WebView2 runtime). If an install needs an admin prompt or a login only I can do, skip every card that depends on it, continue with everything else, and list it under Needs Akshit.
- Missing keys or services for live tests: skip those tests, never block on them.
- A budget you cannot meet after the failure ladder: record the measured number in docs/performance.md, note it in the handoff and keep going.
- Hard limits, never crossed and never asked about: no force push, no `git reset --hard` on pushed history, nothing deleted or written outside the repo folder (except installing tools), no secrets printed, committed or logged.

The only reason to stop the whole build is a broken toolchain that no card can work around. Then write the handoff and stop.

# 10. Session handoff

Usage limits can end a session at any moment and you cannot see how close they are. So the handoff is never written "at the end"; it is kept current all the time.

File: .agent/HANDOFF.md (gitignored like the rest of .agent/). Sections, in this order:

```
# Handoff  ·  session <n>  ·  <date time>  ·  last commit <sha>
## Status       milestone, wave, cards done / total for the wave and milestone, overall health in one line
## Done         this session: card id, title, commit sha, what it now does (one line each)
## In progress  every card dispatched but not committed: agent role, model, files it owns, what is already written, what is left, how to verify
## Left         remaining cards in this milestone, grouped by wave, with dependencies
## Next         the exact next 3 to 6 actions in order, with card ids and models, so the next session can start in one step
## Decisions    every decision made this session and why
## Problems     failures, blocked cards, error excerpts (max 15 lines each), what was tried
## Measured     performance numbers so far against section 4
## Needs Akshit logins, installs, keys or anything only I can do; empty if none
## Resume       the one line I paste to continue
```

When to write it:
1. After every commit: update Status, Done, In progress and Next (small edits, seconds).
2. At the end of every wave and milestone: rewrite the whole file from scratch so it reads cleanly.
3. Immediately, before doing anything else, if you see any usage, rate-limit or context warning, or before compacting the conversation.
4. Every 20 subagent dispatches as a safety net.
5. Also append a dated one-paragraph summary of the session to .agent/SESSIONS.md each time you rewrite the handoff fully, so history survives.

Resuming: when a session starts and .agent/HANDOFF.md exists, you are resuming. Read HANDOFF.md, PLAN.md and the tail of LOG.md, run `git log --oneline -25` and `git status`, and reconcile: uncommitted changes that belong to an in-progress card are either finished and tested or discarded with `git checkout -- <owned files>` and the card re-dispatched. Then continue from Next. Never re-plan from scratch and never redo committed cards.

# 11. Final report

After each milestone, rewrite the handoff and post a short status. When W9 is done, reply with:
1. What shipped against sections 1.1 to 1.3, item by item.
2. The performance table from docs/performance.md, measured against section 4.
3. Blocked cards and followups.
4. Commands for me: run in dev, run tests, cut a release, run laya-serve and Ollama for local use.
5. Everything under Needs Akshit: logins, repo visibility, Vercel domain, optional keys and the Spotify developer app.
