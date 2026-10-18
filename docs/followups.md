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
