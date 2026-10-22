# ADR-012: Prefixes and plain text

Status: Accepted and shipped (2026-10-23)

## Context

Flick began with four modes: `>` Commands, `!` Apps, `/` Find and `?` Ask. Apps, programs and files were three separate places to look for something to open, and math, units and project folders were mixed into Commands, where a short word could match several unrelated things. Users had to know which prefix held what before they could type.

## Decision

- Five prefixes, typed as the first character of the empty bar:

  | Prefix | Mode | Sources |
  | --- | --- | --- |
  | `/` | Open | apps, programs, files, folders, natural-language finder |
  | `>` | Commands | system commands, web shortcuts, media, ports and processes, quick capture |
  | `=` | Calc | math, units, currency, time zones |
  | `?` | Ask | none: Ask never searches, it sends the question to a provider |
  | `@` | Projects | project launcher |

- `!` is retired. Open covers what it did, and `!` stays free to be typed as ordinary text.
- A prefix counts only as the first character into an empty field, typed or pasted. One character, in first position, never later in the text, so a `/` or `?` inside a path or a question is never taken for a prefix.
- The UI owns one prefix table. Each prefix names the sources it sends to the `search` command: `apps`, `files`, or the id of a feature. Rust composes the results from those sources and fails on an unknown id. A known source that is off, or whose index is disabled, adds nothing and is not an error.
- A prefix is offered only while one of the sources it needs is running. The launcher reports the live sources each time it is shown, and the hint list, `Ctrl+1` to `Ctrl+5` and prefix typing all follow it.
- Open merges its sources into one list, in this order: the natural-language row if there is one, apps, programs, files, folders. The list is cut to 8 rows with 3 reserved for files when files exist. Groups are labelled Apps, Programs, Files and Folders.
- Plain text with no prefix searches apps, programs and every enabled feature, but not files, and ends with fallback rows under the label "Search instead":
  - Find files named the text. Enter switches to Open with that text.
  - Search the first web shortcut for the text. Shown only while the web feature runs. Enter opens it.
  - Ask the selected model the text. Shown only while Ask is on. Enter sends it.
- The `links` feature classifies typed text, in plain text and in Commands. It offers one Open row for an http or https URL (a bare domain becomes https), or for an absolute drive path (`C:\...`, `%APPDATA%\...` or `~\...`). It never offers UNC paths, device paths, paths with `..`, other URL schemes, links with a user name in them, or file names that look like domains. It reads nothing but the text while you type, and opens only on Enter.

## Alternatives

- Prefix-only, with no plain-text search. Rejected: a user who types a name with no prefix gets nothing, which reads as broken.
- Suggest mode only, a row that suggests the right prefix for plain text. Rejected: it costs a keystroke and a decision on every search, and still shows no result.
- Default to apps, so plain text acts like the old `!`. Rejected: it hides every feature from plain text, and teaches no prefix.

## Consequences

- Plain-text search runs more Rust work per keystroke, across apps, programs and every enabled feature. Each feature's `query` must stay inside its 0.5 ms budget ([performance budgets](../performance.md); ADR-009 makes `query` synchronous and cheap).
- Discoverability rests on the prefix list in the empty bar and on the footer, since prefixes are no longer chosen by cycling.
- A prefix with no live source disappears from the list and its character types normally.
