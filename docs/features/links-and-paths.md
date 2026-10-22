# Links and paths

Type a link or a folder path and Flick adds one Open row for it. Nothing opens until you press Enter.

## What is detected

| You type | Row |
| --- | --- |
| `https://github.com/x` or `http://localhost:3000` | Open the link |
| `github.com/x` (a bare domain) | Open `https://github.com/x` |
| `C:\Users\me\Docs` or `C:/tmp/a` | Open the path |
| `%APPDATA%\Foo` | Open the path with the variable filled in |
| `~\Downloads` | Open the path in your user folder |

Bare domains must end in a common top-level domain: com, org, net, io, dev, app, ai, co, in, uk, de, fr, us, edu, gov, info, me, tv, xyz, tech, cloud, site or online. Allowed variables are APPDATA, LOCALAPPDATA, USERPROFILE, TEMP, PROGRAMDATA and PROGRAMFILES. Text longer than 2048 characters is ignored.

A path opens only if it exists. Otherwise Flick says "That path does not exist".

## What is never detected

- Network paths (`\server\share`) and device paths (`\?\C:\...`).
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

Settings > Features, "Links and paths".
