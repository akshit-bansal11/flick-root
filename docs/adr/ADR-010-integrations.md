# ADR-010: Integrations

Status: Accepted; implemented in v0.1.0 (`crates/features/src/media/`, `crates/features/src/capture/`)

## Context

Flick can control media, and send quick notes to other tools, without a paid account. Each integration needs its own credentials or a local file path, and each must stay FOSS-first: the user supplies every key, and no integration is required for the launcher to work.

## Decision

Media, base layer (no account):

- Windows Global System Media Transport Controls (`Windows.Media.Control`, through the `windows` crate) for play, pause, next, previous and now-playing on any player, including Spotify and YouTube in a browser.
- "play <song> on spotify" opens `spotify:search:<query>` in the Spotify desktop app.
- "play <x> on youtube" opens the YouTube search URL in the default browser.

Media, optional Spotify connection:

- OAuth PKCE with the user's own Spotify developer app client ID. A shared client ID cannot work for a public app, because since February 2026 development-mode apps need the app owner on Premium and allow 5 users per app.
- Connecting is offered in Settings > Integrations as "unlock more Spotify features". It adds: play a specific song, album, artist or playlist; add to queue; like or save the current track; switch playback device; search the user's own playlists.
- The Spotify Web API docs must be checked at build time for which actions need Premium, and that is shown next to each action.
- Without a connection, the base layer still works.
- Optional YouTube Data API key to open the first result instead of the search page.

Quick capture:

- Targets: Obsidian vault (append to today's daily note, or a chosen file, by writing the Markdown file directly; no plugin), plain Markdown file, Notion (internal integration token plus database ID, with parsed fields mapped to properties), and webhook (POST JSON to a URL).
- Parse "log 250 lunch" style input locally. Ask the Query provider only when the format is unclear.

Through v0.0.1 none of this was implemented. It shipped in v0.1.0.

## Alternatives

- A shared Spotify client ID. Rejected: the Spotify development-mode limits in the decision above make it unworkable.
- A Notion or Obsidian plugin. Rejected: quick capture writes the Markdown file directly, so no plugin is needed.
- Sending every capture to a model for parsing. Rejected: local parsing covers the usual formats, and a model is asked only when the format is unclear.

## Consequences

- No integration is needed for the launcher to work.
- Each integration's secrets follow ADR-011, and its host binding applies.
- Spotify Premium requirements can change; the decision says to check the current docs at build time, not to rely on this text.
- Quick capture must run the secret check on captured text before it is written or sent (ADR-011 rule 8).
