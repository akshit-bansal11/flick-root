# Links and paths

Type a link or a folder path and Flick adds one Open row for it. Nothing opens until you press Enter.

## What is detected

| You type | Row |
| --- | --- |
| `https://github.com/x` or `http://localhost:3000` | Open the link |
| `github.com/x` (a bare domain) | Open `https://github.com/x` |
| `C:\Users\me\Docs` or `C:/tmp/a` | Open the path |
| `%APPDATA%\Foo` | Open the path with the variable filled in (a `\` must follow the closing `%`) |
| `~\Downloads` | Open the path in your user folder |

Bare domains must end in a common top-level domain: com, org, net, io, dev, app, ai, co, in, uk, de, fr, us, edu, gov, info, me, tv, xyz, tech, cloud, site or online. Allowed variables are APPDATA, LOCALAPPDATA, USERPROFILE, TEMP, PROGRAMDATA and PROGRAMFILES. Text longer than 2048 characters is ignored, and so is a link that grows past 2048 characters once Flick encodes it.

A path opens only if it exists. Otherwise Flick says "That path does not exist".

## What opens on Enter

Flick never runs a program from a typed path. Before opening, it looks up the real file the path points to, so short names (such as `REPORT~1.PDF`), symbolic links and junctions are judged by the file they really lead to.

- A folder opens in File Explorer.
- A file opens directly only if its type is on a safe list of documents, pictures, audio, video and archives: pdf, txt, md, rtf, csv, log, json, xml, yaml, yml, toml, ini, doc, docx, xls, xlsx, ppt, pptx, odt, ods, odp, epub, png, jpg, jpeg, gif, webp, bmp, svg, ico, tif, tiff, heic, mp3, wav, flac, ogg, m4a, aac, mp4, mkv, mov, avi, webm, wmv, zip, 7z, rar, tar and gz. Case does not matter, and trailing dots or spaces in the name are ignored.
- Any other file, such as `.exe`, `.py`, `.bat`, `.lnk`, `.appref-ms` or a file with no extension, opens the folder that holds it instead. Nothing in that folder runs.

The row says "Open" for a name on the safe list and "Open folder" for everything else. To start a program, search for it by name or type `/`.

## What is never detected

- Network paths (`\\server\share`) and device paths (`\\?\C:\...`).
- Paths that name a Windows device, such as `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9` or `LPT1` to `LPT9`, with or without an extension, anywhere in the path.
- Any path with `..` in it, or with `< > " | ? *`, or with a second `:` after the drive letter.
- Schemes other than http and https, such as `file:` or `javascript:`.
- Links with a user name or password in them, such as `https://bank.com@evil.com`.
- File names that look like domains, such as `notes.md`, `main.py` or `archive.zip`, because those endings are not in the domain list.
- Text with a line break or other control character.

## Where it works

In plain text (no prefix) and after `>`. It does not run after `/`, `=`, `?` or `@`.

## Privacy and safety

Flick only reads the text while you type. It does not check the network or the disk until you press Enter. On Enter it checks the text again before opening it.

## Turn it off

Settings > Prefixes, "Links and paths".
