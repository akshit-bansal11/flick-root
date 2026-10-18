# ADR-009: Feature modules and toggles

Status: Accepted (2026-10-19)

## Context

Flick ships several optional features (app and file search, math, projects, system commands, the web and others). Each has its own background work: watchers, timers, caches and network clients. A user who turns a feature off must get no CPU, no network and no memory cost from it. Each feature's settings also need a page, and writing a hand-made settings page for each feature does not scale.

## Decision

- Every feature is a module behind one trait in `crates/features`, roughly: `id()`, `title()`, `default_enabled()`, `settings_schema()`, `start(ctx)`, `stop()`, `query(&Input) -> Vec<Item>`, `execute(&Item, Action)`.
- `query` is synchronous and cheap. Anything slow is precomputed in `start`.
- A registry owns the features. Turning a feature off calls `stop()` and drops everything it holds: watchers, timers, caches and network clients.
- A disabled feature costs zero CPU, zero network and near-zero memory.
- Toggles apply instantly, with no restart.
- Find mode and Query mode have toggles. Command mode is always on.
- Config shape: `[features.<id>] enabled = true`, plus that feature's options.
- Each feature's options page is generated from its `settings_schema()`.

In v0.0.1:

- The trait also has `version_tag()`, `description()` and `modes()`, which the Features page uses. These are additions to the KICKOFF list, not replacements.
- `start` takes `FeatureCtx` and returns `Result<(), FeatureError>`. A failed start shuts down its child scope, and the feature is skipped by `query_all` (`crates/features/src/registry.rs`).
- Background work is spawned only through `ctx.tasks`.
- Known gaps: the Web and Math features keep some state in module-level statics because the registration code builds them as unit values. The followups say to move that state into the struct, and `Frecency::clear_history()` does not yet notify the frecency feature's in-memory maps (open followups).

## Alternatives

- Compile-time feature flags. Rejected: the user toggles features at runtime, with no restart.
- A plugin system with dynamic loading. Not needed for v0.0.1: the features are built in.
- A hand-written settings page per feature. Rejected: the schema-driven page removes that work for each new feature.

## Consequences

- A new feature is one module plus one registration line, and gets a settings page for free.
- Stopping a feature must release everything it holds. Tests should show that stop releases the watchers and timers.
- Shared state held in statics breaks this rule until it is moved into the feature struct.
