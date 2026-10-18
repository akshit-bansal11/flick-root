# ADR-006: Providers

Status: Accepted (2026-10-19)

## Context

Query mode sends a question to a language model. Flick is a wrapper: it bundles no models and no keys, and nothing AI-related is on by default. The user chooses every provider. Local servers (Ollama, LM Studio, llama.cpp) and hosted APIs use two wire formats, OpenAI Chat Completions and Anthropic Messages, so one client per format can cover all presets.

## Decision

- One provider trait, roughly `Provider::stream(Request) -> impl Stream<Item = Result<Delta>>`, with two wire formats: OpenAI Chat Completions and Anthropic Messages.
- Presets, in this order: Ollama (http://localhost:11434/v1), LM Studio (http://localhost:1234/v1), llama.cpp server (http://localhost:8080/v1), OmniRoute (http://localhost:20128/v1), OpenRouter (https://openrouter.ai/api/v1), Anthropic, OpenAI, Custom (any OpenAI-compatible base URL, model, optional key). All presets except Anthropic use the OpenAI format.
- On first run and on demand, probe the three local ports with `GET /v1/models` (200 ms timeout) and list the models found. Detected servers are offered, never switched on automatically.
- No provider is enabled by default and none is marked as recommended.
- Query mode uses the first enabled provider that responds and falls back down the user's order on connection errors, 429 and 5xx.
- Fast/Think maps to each provider's own reasoning control where it has one, and is hidden where it has none.
- Anthropic `claude-haiku-5-5`: never send `temperature`, `top_p`, `top_k` or an assistant prefill, because the model rejects them.
- Deltas stream to the UI through `tauri::ipc::Channel`. Esc aborts the request.
- System prompt: answer directly and briefly, markdown allowed, `max_tokens` 1024.

In v0.0.1:

- Preset list matches the order above (`crates/providers/src/presets.rs`, `PRESETS: [Preset; 8]`), with `default_enabled()` returning false for every preset.
- Think uses Anthropic adaptive thinking with `output_config.effort` set to `high`. Fast sends no thinking control (per the build log, LOG.md T2.4).
- The OpenAI-format `Bearer` header string is wrapped in `zeroize::Zeroizing`; the Anthropic `x-api-key` header borrows the secret directly. `HeaderValue` still copies the bytes, which cannot be zeroized.
- Outbound HTTP for providers uses `reqwest` 0.13 with the `native-tls` feature (SChannel on Windows). reqwest 0.13 has no `rustls-tls` feature, so the TRD's rustls choice was replaced (LOG.md, 2026-10-19 decision).

## Alternatives

- One client per vendor SDK. Rejected: two wire formats cover every preset, and a vendor SDK per provider adds dependencies the user did not ask for.
- Auto-enable the first detected local server. Rejected: the user picks every provider, and nothing is switched on without a choice.
- A recommended default provider. Rejected: no provider is recommended, so the choice stays with the user.
- rustls for reqwest. Not available as a feature in reqwest 0.13; native-tls was used instead.

## Consequences

- Adding a new OpenAI-compatible service is a preset entry, not new code.
- Local servers work with no account and no key.
- Any Anthropic change to the request shape needs a change in the Anthropic wire code only.
- The copy of the key inside reqwest's `HeaderValue` cannot be zeroized; this residual copy is accepted against ADR-011 rule 5.
