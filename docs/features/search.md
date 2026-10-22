# Search

How Flick matches what you type. The rules differ by prefix, so each is listed.

## Plain text

Text with no prefix searches apps, programs and every enabled feature, using the rules below. It does not search files. Matching features show their rows, for example `lock` lists the Lock command and `port 3000` lists the port lookup.

The last rows are always under "Search instead":

- **Find files named "text"**: Enter switches to Open and searches files.
- **Search the web for "text"**: uses your first web shortcut. Shown only while web shortcuts are on. Enter opens the search in your browser.
- **Ask**: sends the text to your selected model. Shown only while Ask is on. Enter sends it.

A typed link or drive path also gets an Open row ([links and paths](links-and-paths.md)).

## Apps in Open (`/`)

- Letters must appear in order in the app name (nucleo fuzzy match). `spo` finds Spotify, `vsc` finds Visual Studio Code.
- If nothing matches and you typed 4 or more letters, one wrong letter is tolerated. `spotifu` finds Spotify.
- Open merges apps, programs, files and folders in one list, grouped as Apps, Programs, Files and Folders.

## System commands (`>`) and feature keywords

- Dropped letters are accepted. `slep` finds Sleep, `blutooth` finds Bluetooth.
- From 3 letters up, one wrong or swapped letter is accepted. `shtdwn` finds Shut down, `sleeo` finds Sleep.
- Exact and prefix matches always rank first.
- This covers feature keywords such as `port`, `proc`, `note` and `todo`.

## Files in Open (`/`)

- Letters must appear in order across the file name. `invce` finds `invoice.pdf`.
- File search does not tolerate typos.

## Exact only

Math (`=`, including number bases such as `255 to hex`), time zones and natural-language find are exact. They parse expressions (`2+2`, and natural-language finds such as `the invoice i downloaded last week`), so a fuzzy match would turn a typo into a different answer.

## Ranking

Exact and prefix matches rank above fuzzy-only hits. Fuzzy-only hits need at least 3 characters in the query.
