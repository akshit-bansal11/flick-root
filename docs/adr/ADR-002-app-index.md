# ADR-002: App index

Status: Accepted (2026-10-19)

## Context

Launching an installed app should work the way Start's "All apps" list works. That list includes classic shortcuts and packaged (Store) apps. A cold start must not wait for a full enumeration.

## Decision

- Apps are enumerated from the Shell AppsFolder (`FOLDERID_AppsFolder`) with the `windows` crate (`0.62` in the root `Cargo.toml`). This is the same list as Start's All apps.
- Apps launch with `ShellExecuteExW` on `shell:AppsFolder\<parsing name>`. Admin launch uses the `runas` verb. Admin launch runs synchronously so that a cancelled UAC prompt is reported.
- Entries are filtered out when the name matches `(?i)\b(uninstall|readme|help|documentation|website|release notes|license|manual)\b`. Entries whose target is a folder or a non-executable document are also filtered out.
- Icons are extracted to PNG at 32 and 64 px using the `png` crate (`0.18`).
- The list and icons are cached under `%LOCALAPPDATA%\<app>\cache\`: the list in `apps.json`, icons as `icons\<hash>-32.png` and `icons\<hash>-64.png`.
- The list is held in a lock-free swappable store (`arc-swap`). A refresh runs in the background every 10 minutes and on demand.
- Unsafe code is allowed only in `crates/index-apps` and `crates/win`. Each block has a `// SAFETY:` comment.

## Alternatives

- Scanning Start Menu `.lnk` folders only. Rejected because it misses packaged apps.
- Registry uninstall keys. Rejected because they do not match the Start list.

## Consequences

- Measured on the dev machine: launch call 0 to 1 ms (budget 30 ms). Command match over 2,000 apps 0.25 ms (budget 2 ms).
- Known limit: icon bitmaps come back premultiplied and are stored as straight alpha, so dark icon edges may appear. With `SIIGBF_BIGGERSIZEOK` a "32" icon may be larger than 32 px. Both are open in `docs/followups.md`.
- Live launch was checked on the dev machine with Calculator and Notepad. A packaged Store app (Spotify) could not be checked there.
