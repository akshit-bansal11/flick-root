# Quick capture

Jot a line from the launcher: `log 250 lunch #food`, `note call the bank`, `todo buy milk`. Flick parses it offline and writes it to Obsidian, a Markdown file, Notion, or a webhook you set up.

## How to use it

Type `log`, `note` or `todo` followed by text, after typing `>`. One row appears for each target you have set up; pick one and press Enter. With no target set up, the row says "Set up quick capture" and points to Settings, Integrations.

- `log 250 lunch #food yesterday 12:30`: finds an amount (`250`, `$12.50`, `1.5k`), `#tags`, a date (`today`, `yesterday`, `tomorrow` or `2026-10-19`) and a time (`HH:MM`, 24 hour). What is left is the text.
- `note ...` and `todo ...`: the text is kept as typed (tags are still read).

What is written to a Markdown target:

- log or note: `- 12:30 lunch (250) #food` (the time is the current time unless you gave one; tags are written only for logs)
- todo: `- [ ] buy milk`

## Targets (Settings, Integrations, Quick capture)

- **Obsidian:** set your vault folder, an optional daily note folder (inside the vault) and the format (default `YYYY-MM-DD`; `YYYY`, `YY`, `MM`, `DD` are understood). Flick appends to the daily note file directly; no plugin is needed. A daily folder or note that resolves outside the vault through links is refused before anything is created.
- **Markdown file:** appends to a file you choose; its folder must already exist.
- **Notion:** create an internal integration in Notion, share your database with it, then paste the token and the database ID in Settings. Flick reads the database once and fills what it has: the title property from the text, the first number property from the amount, the first date property from the date (default today), and the first multi-select property from the tags. Missing properties are skipped; a database with no title property is rejected. Requests use Notion API version `2026-03-11`.
- **Webhook:** Flick POSTs JSON `{"text", "fields": {kind, amount, tags, date, time}, "time"}` to a URL you add. The URL is a secret, so it is stored in Credential Manager, and its host must match the host you entered. Network errors and 5xx replies are retried up to twice (after 1 s and 3 s).

Each target has a **Test** button. For webhooks it is **Send test**, which does post one test message. Stored secrets: `integration.notion@api.notion.com` and `integration.webhook.<name>@<host>`.

When a `log` line has no amount or no label, Flick may ask your Query provider (if one is enabled) to fill the empty fields: amount, tags, date. Its reply is parsed strictly, anything extra rejects the whole reply, and it can only fill fields, never trigger an action. If no provider is enabled, or it fails, the raw text is captured.

## Privacy

- Obsidian and Markdown targets are local files. Nothing leaves your machine.
- Notion and webhooks send the captured entry to that service, and only if you set them up. Secrets are write-only from the UI and bound to their host.
- The optional fill step sends the text of the line to your Query provider, only when the line is incomplete and a provider is enabled.
- Text that looks like a secret or API key is **blocked** before it is written or sent; the row says "Blocked: looks like a secret".

## Limits

- Dates are `today`, `yesterday`, `tomorrow` or `YYYY-MM-DD`; times are `HH:MM`.
- Only the first amount, date and time in a line are read.
- Notion needs the database shared with your integration; Flick does not create properties.
