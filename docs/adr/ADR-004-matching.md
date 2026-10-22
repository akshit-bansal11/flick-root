# ADR-004: Matching

Status: Accepted (2026-10-19)

## Context

Command mode and Find mode both need fuzzy matching that is fast on every keystroke. Results from apps, files and features must be ordered together. Matching must work offline.

## Decision

- Both modes use `nucleo-matcher` (`0.3`). `crates/launcher-core/src/matcher.rs` wraps it.
- Haystacks are converted to UTF-32 once, when the list changes. Each keystroke parses the pattern once and keeps the best `k` hits in a small heap.
- Final rank is the match score plus feature boosts. Frecency and prefix learning are boosts from the Frecency feature, applied only when that feature is on.
- Score normalisation: nucleo scores are unbounded, but feature items score 0 to 1. App scores are scaled relative to the top app score and capped at 0.9. An exact feature keyword (score 1.0) therefore outranks a fuzzy app hit, while apps still order among themselves.
- Ties break on shorter title, then alphabetical order.
- Matching never calls the network.

## Alternatives

- Substring matching. Rejected because fuzzy matching is expected for app names.
- A separate ranker per mode. Rejected because results must sort together.

## Consequences

- Measured on the dev machine: Command match over 2,000 apps with every feature on, worst case 0.29 ms (`2.5 kg to lb`), budget 2 ms.
- Each feature's `query()` is budgeted at 0.5 ms. The math feature is closest, at 295 µs.
- Known limit: app-name matching enables fzf syntax (`!`, `^`, `'`, `$`).
- Known limit: a weak project match can outrank an exact app match.
