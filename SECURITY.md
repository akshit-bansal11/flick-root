# Security policy

## Supported versions

Flick is pre-1.0. Only the latest published release receives security fixes. Older releases are not patched; update to the latest release.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Report it privately through GitHub's private vulnerability reporting: open the **Security** tab of the repository and choose **Report a vulnerability** (a security advisory). Include what you found, the version, the steps to reproduce, and the impact you expect. The maintainer will reply through that advisory.

## Threat model

The rules below come from [ADR-011](docs/adr/ADR-011-secret-security.md). They limit where a provider key can live, who can read it, where it can be sent, and what can leak it.

- **Storage.** Provider keys are stored in Windows Credential Manager, which is protected per Windows user. Keys are never written to `config.toml`, SQLite, logs, crash output, exports or backups.
- **Write-only.** The UI hands a key over once. No command returns a stored key. The UI sees only whether a key is present, its last four characters, its host, and when it was added and last used.
- **Host binding.** A key is bound to the host it was entered for. It is sent only over HTTPS to that host. Plain HTTP is allowed only for `localhost` and `127.0.0.1`. Authenticated requests do not follow redirects. Changing a provider's host removes the stored key.
- **Network location.** All network calls are made by the Rust side. The WebView makes no network calls, and its Content Security Policy blocks remote scripts, frames and images.
- **Redaction.** Logs and the panic hook mask key-like patterns and every stored secret value.
- **Signed updates.** Updates are signed with the maintainer's updater key (minisign, via Tauri). Flick refuses an update whose signature does not match the key built into it. This is separate from Windows code signing.
- **Untrusted output.** Model output is rendered as sanitized Markdown with raw HTML off and no remote images. It cannot trigger any action by itself.
- **Secret-looking text.** Text that looks like a secret is blocked from being sent to a model.
- **Warning.** Adding a key shows a risk warning, and saving requires confirmation.

## Residual risk

No storage is completely safe. The following risks remain and are accepted:

- Malware running as your Windows user can read Credential Manager, and read keys from memory while Flick uses them.
- A provider breach, or a compromised provider account, can expose a key.
- Some copies of a key exist inside the HTTP library while a request is built and cannot be zeroized.
- Some CSP and redirect-policy deviations are recorded as open gaps in ADR-011.

Keep provider keys dedicated to Flick, set a usage limit at the provider, and revoke a key there if you think it leaked.

## Out of scope

- Malware or another program running as the same Windows user. Flick cannot protect secrets from code that already runs as you.
- Compromise of a provider's own service or account.
- Physical access to an unlocked machine.
- Problems in a model's answers. Model output is untrusted by design and is not a security boundary.
