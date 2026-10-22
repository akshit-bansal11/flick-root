# ADR-007: Storage and secrets

Status: Accepted (2026-10-19)

## Context

Flick stores settings, usage history and caches on the user's machine. Some of that data is sensitive: provider keys and OAuth tokens. Settings and caches are not. The split has to be clear so that secrets never end up in a file that is easy to read, copy or back up.

## Decision

- API keys and OAuth tokens live in Windows Credential Manager through the `keyring` crate, and follow ADR-011.
- Secrets never go into config files, logs, panics, test snapshots, or requests to decision engines.
- Config is TOML at `%APPDATA%\<APP_NAME>\config.toml`.
- Frecency, prefix learning and caches use SQLite through `rusqlite` with the `bundled` feature.

Current state:

- `keyring` 4.2 with the `windows-native-keyring-store` feature (root `Cargo.toml`).
- Secret metadata (name, host, `added_at`, `last_used`; never values) is stored in `secret_meta.json` under the local data directory, not in SQLite. `Store` has no `secret_meta` accessors yet, so the table is pending. Metadata is written by `apps/desktop/src-tauri/src/meta_store.rs`.
- `last_used` is recorded in memory on each key read and flushed in coalesced writes at most every 5 seconds from a background thread, plus a `SecretVault::flush()` call on shutdown (`crates/providers/src/secret.rs`).
- Key lookup does not block the async runtime: `key_for` runs inside `spawn_blocking`, and `authorize` is async.
- Keys stored earlier are not registered with the log redactor until they are set again.

## Alternatives

- Keep keys in `config.toml`. Rejected: the file is readable, copyable and easy to back up, and ADR-011 forbids it.
- Store secret metadata in SQLite from the start. Not done: the store lacked accessors, so a JSON file was used. The SQLite table is the planned replacement.
- Use a per-app encryption key on disk instead of Credential Manager. Rejected: DPAPI through Credential Manager is per Windows user and needs no key file.

## Consequences

- Deleting the Windows user's Credential Manager entries removes keys without touching config or the database.
- Tests that touch secrets must use a mock or temporary entry names; a real key must never appear in a fixture.
- The `secret_meta.json` file is a second persistence point until the SQLite table lands. Its contents must stay free of values.
