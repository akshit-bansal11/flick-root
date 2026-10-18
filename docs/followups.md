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
