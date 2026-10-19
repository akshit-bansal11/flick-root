# Natural-language finder

Find a file by describing it, for example `the invoice i downloaded last week` or `meeting notes from last month on desktop`. Flick parses the phrase offline, searches its file index, and opens the file only when the answer is clear and the file is a safe type. Otherwise it lists the best candidates.

## How to use it

In Find mode, type a plain-language phrase. A phrase counts as natural language when it has three or more words, or contains one of: `my`, `from`, `in`, `the`, `that`, `last`, `yesterday`. Flick adds a row `Find: <phrase>`; press Enter to run it. Shorter inputs stay ordinary fuzzy file search.

What the offline parser understands:

- File types by word: photos, pictures, images, screenshots, pdf, sheets, spreadsheets, excel, docs, documents, word, slides, decks, archives, and more.
- Places: Desktop, Documents, Downloads, Pictures, Music, Videos (the folder must be one of your indexed roots).
- Dates: `yesterday`, `this week/month/year`, `last week/month/year`, `last 7 days`, a weekday name, a month name.
- Filler words (`open`, `find`, `my`, `the`, `called`, ...) are dropped; the rest is matched against file names.

Examples: `pdf from yesterday`, `that deck from last month`, `find the pdf called taxes from last year`.

## When it opens a file

Stage 1 (no engine involved): the top hit is opened when it is a file (not a folder), its match score is at least 80, it beats the runner-up by at least 10%, and its extension is on the allowlist: images (jpg, jpeg, png, webp, heic, gif, bmp), spreadsheets (xlsx, xls, csv), documents (docx, doc, odt, rtf, txt), slides (pptx, ppt, odp), video (mp4, mkv, mov, avi, webm), audio (mp3, flac, wav, m4a, ogg), pdf, and md. Executables, scripts, shortcuts, archives and everything else are never auto-opened; they appear in the list.

Stage 2 (only if you enabled an engine): when the parser is not sure, the enabled decision engines are asked to pick one candidate. An answer is accepted only if its probability is at least the threshold (default 0.6) and the file passes the same allowlist. The first engine that answers ends the chain, whether or not its answer is accepted.

Stage 3: if nothing was chosen, the candidates are shown as a normal Find list.

A file that was opened is remembered for the same phrase; the cached file is reopened only if it still exists and is still an allowlisted type.

## Decision engines

Settings, Decision engine. All engines are **off by default**; the built-in parser always runs first. Order: Laya, Kev, your AI provider, Jev, Custom.

| Engine | Default address | Notes |
| --- | --- | --- |
| Laya | `http://localhost:8000` | Meant to run on your machine. No key needed. |
| Kev | empty | Set the base URL of a server you run. |
| Your AI provider | none | Uses your first enabled Query provider (AI providers page). |
| Jev | `https://api.typesafe.ai` | Hosted service. Needs your own API key; default timeout 4000 ms. |
| Custom | empty | Any server that speaks `/v1/systemone`. Set its base URL before enabling. |

Limits (Decision engine page): candidates sent, 1 to 8 (default 8; Laya's accuracy drops with many options), and threshold, 0 to 1 (default 0.6). Timeout per engine is 100 to 60,000 ms (default 3000 ms; Jev's default is 4000 ms). A base URL must be https, or http for localhost. Keys are stored in Windows Credential Manager under `engine.<id>@<host>`, are write-only from the UI, and are sent only to the host they were entered for. Changing the host deletes the old key.

Laya, Kev, Jev and Custom speak the `/v1/systemone` API described at https://docs.typesafe.ai/api.md.

### Running laya-serve locally

The Settings page says to start `laya-serve` from the Laya package. This page does not give install or run commands, because I could not find them in an official source (the API docs at https://docs.typesafe.ai/api.md do not mention Laya or `laya-serve`). Check the official Laya README for how to install and start it, then point the Laya engine at the address it serves (the default here is `http://localhost:8000`). The status shown on the Decision engine page only checks that the address answers over HTTP.

## Privacy

- With every engine off, nothing leaves your machine. The parser and the search are local.
- With an engine on, the request contains the phrase you typed and, for up to the candidate count of results, each file's **name and parent folder**. Never file contents, and keys are sent only as the Authorization header to their own host.
- Laya is meant to run on your machine. Jev is a hosted service at the address above. "Your AI provider" sends the same names to the provider you chose (for a local provider such as Ollama that stays on your machine).
- Model output is untrusted: an engine can only pick one of the candidates; it cannot run anything, and the allowlist still applies.
- No redirects are followed when a key is sent.

## Limits

- Only the indexed roots are searched, and results are capped (20 hits per root).
- English phrases only; the date and type vocabulary is the list above.
- No engine health endpoint is documented, so the status check is only "the address answers".
