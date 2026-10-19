# ADR-005: Natural-language finder

Status: Accepted; implemented in v0.1.0 (`crates/decision`, `crates/features/src/nlfind/`)

## Context

Find mode works on words typed as a file name. Users also phrase requests in plain language, such as "my photo from yesterday". Such requests need parsing before matching. Model calls are slow and can send data off the machine, so they must stay off the keystroke path.

## Decision

- The finder is two stages, parser first and decision engine second. It runs on Enter in Find mode, never while typing. It runs when the input has 3 or more words or a natural-language marker (`my`, `from`, `in`, `the`, `that`, `last`, `yesterday`).
- Stage 1 is always on and runs offline. A built-in parser removes filler words. It maps location phrases to roots (for example "from pictures" to the Pictures root). It maps file-type words to extensions (`photo`, `image`, `pdf`, `sheet`). It maps time words to modified-date filters (`yesterday`, `last week`). It then runs the normal fuzzy match inside those filters. If the top result scores above the threshold and is at least 10% ahead of the second, it opens.
- Stage 2 runs only when stage 1 is ambiguous. The top 8 candidates are sent, as file name and parent folder only, to the first enabled and reachable decision engine. The request is a single Choice question. The result is acted on when p is at least 0.6. Otherwise the ranked choices are shown. The chosen item is cached against the input.
- Engines are all off by default, in this order: Laya, Kev, the user's Query provider (strict JSON pick), Jev, and Custom. Laya, Kev and Jev share one client for `POST /v1/systemone`, with a configurable base URL and optional bearer key. The protocol is taken from the vendor documentation when implemented.
- Executables and scripts (for example `.exe`, `.bat`, `.cmd`, `.ps1`, `.lnk`, `.msi`) are never opened from a natural-language request. They always go to the choices list.
- Model output is untrusted and cannot trigger an action on its own.

## Alternatives

- A language model for every request. Rejected because of latency, cost and privacy.
- Auto-open for any top match. Rejected for executables and scripts.

## Consequences

- This ADR shipped in v0.1.0. Through v0.0.1 the `crates/decision` crate held a doc comment only.
- With every engine off (the default), the parser and the plain choices list still work.
- Keeping the candidate count at 8 is deliberate. Laya's accuracy drops with many options and is weak without fine-tuning.
