# ADR-003: File index

Status: Accepted (2026-10-19)

## Context

Find mode must search personal files quickly, with no external search service. The index must start instantly after a restart and stay current while the app runs.

## Decision

- Flick keeps its own index in `crates/index-files`.
- The initial scan uses `jwalk` (`0.9`) to walk the default roots in parallel. Default roots are Desktop, Documents, Downloads, Pictures, Videos and Music, plus user-added roots.
- Live changes come from `notify` (`8.2`, stable). Flick pins the stable release, not `9.0.0-rc`.
- Entries are kept in a compact in-memory list.
- Removals are recorded in a bitset overlay. Queries run against the stored list with removed entries masked out. The overlay is rebuilt lazily on the first query after a change. Once the overlay grows past `COMPACT_AT`, it is compacted in a background thread.
- A snapshot of the list is persisted, so a cold start loads it without a rescan.
- The scan skips `node_modules`, `.git`, `target`, `dist`, `build`, `.next`, `AppData`, `$Recycle.Bin`, and hidden and system entries.

## Alternatives

- Windows Search index. Rejected in the original spec because it adds a dependency on a system service with its own ranking.
- A full database index. Rejected for the cold-start and memory budgets.

## Consequences

- Measured on the dev machine over 305k paths: Find match p95 at most 4.7 ms, and at most 3.9 ms with a 2k-entry overlay. Budget 25 ms.
- Removals cost one O(n) pass after each change. The auto-compact thread is not yet covered by a test.
- Known limit: a panic inside compaction can leave the busy flag set.
- Known limit: the scan's arena overflow path can attach children to the wrong parent.
