# ADR-011: Secret security

Status: Accepted (2026-10-19)

## Context

Flick handles provider keys, OAuth tokens, webhook secrets and similar credentials. Residual risk is never zero: malware running as the same Windows user can read Credential Manager, and a provider breach or a hostile extension can leak keys. The rules below limit where a secret can live, who can read it, where it can be sent, and what can leak it. They apply to every secret: provider keys, OpenRouter, TypeSafe, Notion, YouTube, Spotify tokens and webhook secrets.

## Decision

1. Storage: Windows Credential Manager (DPAPI, per Windows user). Never in `config.toml`, SQLite, logs, crash output, exports or backups. Entry names include the provider id and its host, as defined in the spec, for example `provider.openrouter@openrouter.ai`.
2. Write-only from the UI: the WebView hands a key over once through `set_secret`, and Rust stores it. No command ever returns a secret. There is no `get_secret`. The UI receives only `{present, last4, host, added_at, last_used}`.
3. All network calls are made in Rust, never from the WebView. Tauri CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; script-src 'self'`; no remote scripts, frames or images. Tauri capabilities are minimal: no shell, fs, http or opener plugin is exposed to the frontend beyond the exact commands it needs.
4. Model output is untrusted: Markdown is rendered with raw HTML disabled and sanitized, with no remote images, and links open in the browser only on click. Model output can never trigger an action by itself.
5. In memory: `secrecy::SecretString` with `zeroize`. Secrets are not `Debug` or `Display` printable and are dropped as soon as the request is built.
6. A key is bound to the host it was entered for, and is sent only over HTTPS to that host (plain http only for localhost and 127.0.0.1). Changing a provider's base URL host deletes the stored key and asks for it again. Authenticated requests do not follow redirects.
7. A tracing redaction layer masks key-like patterns and every stored secret value. The panic hook redacts too.
8. Explain this and Quick capture block text that looks like a secret.
9. CI runs cargo-deny, cargo-audit and pnpm audit; lockfiles are committed; releases ship SHA-256 checksums.
10. Every stored key has Test, Replace and Delete buttons and shows when it was last used.
11. Residual risk is never zero, so every Add key dialog shows the warning below, and Save stays disabled until the checkbox is ticked. Local providers without a key never show it:

> Your key is stored in Windows Credential Manager and is only sent to `<host>`. No storage is completely safe: malware on your account, a compromised extension or a breach at the provider could still expose it. Add it at your own risk. Before saving, set a spending or usage limit in your provider's dashboard, and use a key created only for Flick so you can revoke it on its own.
>
> [ ] I understand the risk and have set a usage limit.

In v0.0.1:

- The warning text above is the text in `apps/desktop/src/settings/AddKeyDialog.tsx`, with the checkbox label "I understand the risk and have set a usage limit.". The product name in the warning comes from `APP_DISPLAY_NAME` in `apps/desktop/src/brand.ts`; the Rust brand constants live in `crates/launcher-core/src/brand.rs`, and the bundle name in `tauri.conf.json` (`productName`).
- `set_secret` also takes `acknowledged: bool` and rejects `false` (`apps/desktop/src-tauri/src/commands/secrets.rs`), so the rule holds even if the UI is wrong.
- The shipped CSP in `apps/desktop/src-tauri/tauri.conf.json` is: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'`. Two directives go beyond rule 3: `style-src 'self' 'unsafe-inline'` and `font-src 'self'`. This is a known deviation. An open followup asks whether `'unsafe-inline'` is needed, since React style props go through CSSOM and are not blocked by CSP.
- Rule 5 in v0.0.1: the `Bearer` header string in `openai_compat.rs` is wrapped in `Zeroizing`, and the Anthropic header borrows the secret without an intermediate `String`. The copy inside reqwest's `HeaderValue` cannot be zeroized; that residual copy is accepted.
- Rule 6 in v0.0.1: authenticated provider clients use `redirect::Policy::none()`. The shared `ctx.http` client in `apps/desktop/src-tauri/src/state.rs` is used only for unauthenticated fetches (the ECB rates file), so rule 6 does not apply to it; it still uses the default redirect policy and no connect timeout, and an open followup moves it to `providers::http::plain_client()`.
- Rule 7 gap: the panic hook is not tested from outside the process; the unit test covers only the panic text redaction (open followup, T5.10).
- Rule 8 is implemented by a Rust check that blocks AWS, GitHub, `sk-`, Slack, JWT and PEM patterns, and high-entropy tokens of 32 or more characters (`docs/spec/02-TRD.md` section 7.5).
- Key metadata is stored in `secret_meta.json`, which holds no values (ADR-007).

## Alternatives

- Keep keys in config or SQLite for simplicity. Rejected: ADR-011 rule 1 forbids it, and both files are easy to read or back up.
- Let the WebView read keys back to show them. Rejected: rule 2 forbids any command that returns a secret.
- Let the WebView make the network calls. Rejected: rule 3 keeps every network call in Rust.
- Allow authenticated redirects. Rejected: a redirect can send the key to another host (rule 6).
- An override button for the secret check. Rejected: blocked means blocked; the user selects less text.

## Consequences

- A user who forgets the key on a changed host must enter it again. That is intended.
- The redaction layer must be kept up to date when a new key format is added.
- The known gaps above (CSP directives, the residual `HeaderValue` copy, the default-redirect unauthenticated client) stay open until they are fixed. This ADR records them; it does not claim they are closed.
