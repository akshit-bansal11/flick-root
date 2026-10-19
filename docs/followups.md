# Followups

Reviewer NITs, deferred on purpose. Each line: source, location, note.

## W1 review (bdb6106..5c1219e)

- index-apps/src/cache.rs:25 repeats the local data dir logic; use `launcher_core::config::local_data_dir()?.join("cache")`. Cache and icon paths differ from BSD §6.3 (`apps\list.json`, `apps\icons\32\`); the card's layout is what shipped.
- launcher-core/src/store.rs:340 maps a rename failure during reset to `CoreError::ConfigIo`; the variant name is misleading.
- index-apps/src/icons.rs: `GetImage` returns premultiplied BGRA but PNG stores straight alpha (dark icon edges); with `SIIGBF_BIGGERSIZEOK` a "-32.png" may be larger than 32 px.
- tauri.conf.json CSP `img-src 'self' data:` blocks `IconRef::File`; the UI must receive data URLs (decided in T2.7/T3.5).
- launcher-core/src/matcher.rs: `Pattern::parse` enables fzf syntax (`!`, `^`, `'`, `$`) for app names; consider `Atom::new` plain fuzzy.
- src-tauri/src/hotkey.rs:13: every registered shortcut toggles the launcher; the Explain hotkey (F09) needs its own dispatch.
- desktop reducer: first Enter on a guarded command emits `launch`, the second `confirm`; Rust must enforce the two-press guard in both commands.

## W2 review (5c1219e..9e653b6)

- apps/desktop/src/query/markdown.test.ts: add isSafeUrl cases for `JaVaScRiPt:` and `java\tscript:` (behavior already correct via WHATWG URL).
- apps/desktop/src/query/Markdown.tsx:33: image alt text uses the `md-link` class and looks clickable.
- apps/desktop/src/launcher/launcher.css:129: `10.5px` font size literal should be a token.
- apps/desktop/vite.config.ts: `dedupe` + createRoot test workaround hide duplicate React copies (T3.0 pins one version).
- Cargo.toml: reqwest switched from rustls-tls to native-tls (+ form, query); record in ADR-006/011 docs and architecture.md.
- crates/index-files/src/scan.rs:104-114: when `b.push` returns None the stack isn't pushed and children attach to the grandparent (arena overflow only).
- index-files/src/watch.rs: compaction busy flag is reset only after compact() returns; a panic inside compact would block further compactions (use a drop guard).
- crates/features/src/web/mod.rs keeps shortcut state in a module-level static RwLock because lib.rs built Web as a unit value; move it into the struct (like Frecency::default()) — two instances would share state.
- T3.9: secret metadata lives in local_data_dir()/secret_meta.json (name, host, added_at, last_used only) because Store has no secret_meta accessors; move it to the SQLite table.
- T3.9: keys stored in an earlier session are not registered with the log redactor until set again (VaultKeys has no register-on-use hook); add one.
- T4.1: Settings must call `Frecency::clear_history()` for the "Clear history" button; learned prefixes from earlier sessions apply from the second ranking of that prefix (lazy lookup).
- T3.9: set_secret registers the value with the redactor before validation (rejected values are registered too; harmless). If provider_cfg() returns None on a base_url change, key deletion is skipped silently — make it an explicit error.
- T4.5 syscmd: confirm token comes from std RandomState + counter (fine as an accidental-press guard, not a secret); confirm://expired timer path untested (needs a real FeatureCtx).
- T3.7: clear_history clears the store and emits `history://cleared`, but the frecency feature's in-memory maps don't listen yet (the registry can't downcast); add a listener or a registry hook so cleared history disappears without a restart.
- T3.7: the Explain hotkey is saved to config but not registered (global-shortcut handler toggles the launcher for every shortcut) — F09 must add per-shortcut dispatch.
- Test temp-dir names in apps/desktop/src-tauri/src/icons.rs:60 and meta_store.rs:100 contain the brand literal; derive from brand::APP_NAME.
- T4.3 math: '2.5 kg to lb' takes ~295 µs (budget 0.5 ms; fend parse + unit lookup dominate). A hand-rolled fast path for simple unit conversions would cut it if the end-to-end Command bench (T5.6) needs headroom.
- T4.7 projects: VS Code recents come only from %APPDATA%\Code; on this PC VS Code's store has no recents key while Cursor's has 102 (same format). Read the chosen editor's store (Cursor, VSCodium) too.
- T4.7 projects: the `&`/spaces test checks only the built argv; add a runtime test that spawns a temp .cmd with `&`, `"` and `%` in the folder name and asserts std either escapes them or returns an error (relies on Rust ≥ 1.77.2 .cmd arg escaping).
- W3 review NIT: tauri.conf.json CSP adds `style-src 'self' 'unsafe-inline'` beyond ADR-011 rule 3; React style props go through CSSOM (not blocked by CSP) — check if it is needed, else record the deviation in the ADR.
- W3 review NIT: capabilities/default.json:27 lets the launcher window call probe_providers (keyed probes); drop unless the Query empty state needs it.
- W3 review NIT: state.rs:190 ctx.http uses the default redirect policy and no connect timeout; use providers::http::plain_client().
- W3 review NIT: frecency/mod.rs:121 stores kind via Debug lowercase while launch.rs used serde; pick one.
- F3.1: Enter on a math/currency result copies and returns Outcome::Copied, but reducer.ts outcome() has no 'copied' case, so the launcher shows no feedback; add a brief 'Copied' state (W5).
- T4.4 math: run state (rates task + loaded rates) lives in a process-wide static because lib.rs builds Math as a unit value; move into the struct with Math::default() like Frecency (same as the Web followup).
- T4.4 rates: the 64 KB body cap is checked after resp.text() reads the whole body; stream with a capped reader if the endpoint ever becomes configurable.
- F3.2 ranking: app scores are scaled relative to the top app (≤ 0.9), so a weak project match (s/(s+200) ≈ 0.67) can outrank an exact app match scoring ~75% of the top app; rank boosts apply after. Tune with real data in T5.6 (e.g. exact-prefix bonus for apps).
- F3.3: icons::ICON_PX (pub, icons.rs) and a private ICON_PX = 32 in commands/search.rs duplicate one value; use the icons const.
- F4.2/T4.4: the math bench's currency case runs with no rates loaded, so it times the reject path; T5.6 must seed rates for a real conversion timing.
- F3.6: no hub test drives NoKey / HostMismatch / InsecureUrl through stream(); add one with an erroring KeySource expecting Error "provider_error" and no Fallback. Hub::run also sends Started for providers[0] before stream() runs, so Started can precede an authorize failure.
- T5.2: General.tsx has an inline `.s-switch` button duplicating settings/Toggle.tsx; switch General to Toggle.
- T5.3: provider enable control is a native checkbox (Biome rejected role="switch" without aria-checked); restyle to the Win11 pill via Toggle.tsx for consistency.
- T5.10: install_panic_hook itself is not tested from outside (logging is private and the hook is process-wide); unit test covers panic_text redaction only.
- T5.12: SettingsRoot's settings://navigate listener has no test of its own (resolveView is unit-tested).
- W5 review NIT: math calc.rs:129 heavy-input test bound loosened to 5 s; tighten to ~10x BUDGET.
- W5 review NIT: icons.rs:385 data_url clones entry.id per row on the keystroke path, and warm() memoises every icon with no cap (was 512); key the memo by Arc<str> or a borrowed id.
- W5 review NIT: measure.ps1:56 single-instance check matches only the same exe path (an installed copy elsewhere would catch the hotkeys); .DESCRIPTION says SendInput but the script uses keybd_event.
- W5 review NIT: firstrun/Detect.tsx:9 TITLES duplicates Rust preset titles; return the title in ProbeResult.
- P5.3: WebView2 GPU-process commits 700–1,500 MB private during startup; test `additionalBrowserArgs` (keep Tauri's default --disable-features list, add --disable-gpu / --in-process-gpu) with release builds and measure.ps1. Launcher-only idle after settle ≈ 410 MB WS / 290 MB private vs the 120 MB budget; WebView2 alone may exceed it (KICKOFF §9: record if unmet).
- Pre-release review NIT: settings.rs:94 re-registers the hotkey before update_config; if saving fails, the OS binding and config.toml disagree until restart (roll back to the old binding on error).
- Pre-release review NIT: web lib/markdown.ts:9 scheme check misses tab/newline inside the scheme; strip [\t\n\r] first (trusted repo content, low risk).
- Pre-release review NIT: web lib/markdown.ts:12 relative links to files not on the site (LICENSE, docs/adr/, followups.md) become broken site paths; point them at REPO_URL/blob/main/<path>.
- Pre-release review NIT: apps/web/package.json @types/node ^20 while CI uses Node 24; bump to ^24.
- T6.6 web: the landing page's live launcher demo (typing, Tab, prefixes, arrows, autoplay, reduced motion) was never exercised in a browser; pages are HTTP-verified only.
- Laya/Kev: verify they accept the /v1/systemone body and model name '<id>-latest' (docs.typesafe.ai documents Jev only; Laya README not found).
- nlfind: a named root that isn't indexed returns an empty list (fall back to unrestricted search?); engine chain stops at the first engine that answers; auto-open score threshold 80 is uncalibrated.
- ports: confirm-token logic duplicated from syscmd (extract a shared helper); kill errors only reach console; killed row stays until next search; real port/sysinfo path not run live.
- Spotify: verify live — full PKCE connect, NO_ACTIVE_DEVICE in error.reason, whether PUT /me/library needs more than user-library-modify; playlist <q> searches only the first 50 playlists.

## v0.2.0

- Device check: acrylic resize smoothness as the launcher height follows results. Fallback: Mica or a fixed-height window.
- Program watcher: programs are scanned at startup only; no rescan on install.
- Idle RAM with programs loaded (up to 50,000 entries) is not measured against the 120 MB budget.
