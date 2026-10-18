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
