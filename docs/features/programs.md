# Programs

`!` lists your installed apps and, below them, programs Flick finds by scanning every fixed and removable drive.

## How to use it

Type `!` first in the empty bar, then part of a name. Installed apps rank first. A program row shows "Program" and its folder. Enter opens it. `Ctrl+Shift+Enter` runs it as admin, only for `.exe` and `.msi` files. Programs on a removable drive are labelled "Removable" and can never run as admin.

## What is scanned

- Drives: every fixed drive and every removable drive (D:, USB sticks and so on). Network, optical (CD/DVD) and RAM drives are never touched.
- Extensions: exe, msi, msix, msixbundle, appx, appxbundle, apk, xapk, aab, bat, cmd, ps1, lnk, url, appref-ms, jar, appinstaller, app, pkg, dmg, appimage, deb, rpm, flatpak, flatpakref, snap, run, sh.
- A Start menu shortcut for an installed app is merged with that app, not listed twice.
- A loose program never outranks an installed app that matches the same search.
- At most 50,000 programs per scan, shared evenly across the drives scanned together; a drive scanned on its own can fill all 50,000. Settings shows the count and says when a scan hit the cap.

## Excluded folders

Settings, General, "Excluded folders" controls what is skipped. Every default can be removed. Each drive also has a checkbox there.

Default exclusions, and why:

| Entry | Reason |
| --- | --- |
| `winsxs` | Duplicate system files, hard-linked, far too many to be programs. |
| `node_modules` | Thousands of per-project script shims (`.cmd`). |
| `recent` | Windows Recent holds a link for every document you opened. |
| `site-packages` | pip's launcher stubs (`t32.exe`, `w64.exe`, `cli.exe`), not programs. A venv's own `Scripts\python.exe` is still indexed. |
| `target\*\deps`, `target\*\build`, `target\*\incremental` | Cargo intermediates (hashed test binaries and build-script programs). `target\release` and `target\debug` stay indexed. |

Always skipped, and not removable: `$Recycle.Bin`, `System Volume Information`, `$WinREAgent`, `$SysReset`, `Config.Msi`, and any folder with the System attribute.

An entry can be a drive (`E:\`), a folder (`D:\Games`), or a folder name or pattern (`node_modules`, `.*`, `target\*\deps`). The same list applies to Find, so an excluded folder is excluded from both.

## When it is scanned

Flick does not scan on a timer.

- At startup the saved index loads with no scan. The first run builds it once.
- When the launcher opens, apps refresh if they are older than 10 minutes. A newly attached or swapped drive is scanned, and only that drive. A full programs rescan runs if the last one is older than 24 hours.
- A new or deleted program under a watched folder (your user folders and indexed drives) appears or disappears within about 2 seconds.
- Settings, Reindex rescans everything.
- Portable programs on the system drive outside your user folders are not watched. They appear after the daily rescan or after Reindex.

## Removable drives

- A drive that is unplugged has its programs hidden. They come back at once when the same drive returns, with no rescan.
- A different stick on the same drive letter is rescanned.

## Matching

A program is listed only if its name contains what you typed, or every word you typed starts a word in its name. Scattered fuzzy hits are dropped, so an abbreviation such as `vsc` does not match programs.

## Notes

- Icons are loaded only for local `.exe` files. Other types, and files on network paths, get a generic icon, to avoid reaching out to a network share (NTLM credential exposure).
- Mac and Linux package types (dmg, deb, rpm and so on) open with whatever Windows associates with them; Flick does not install them.

## Turn it off

Settings, General, "Find programs on all drives" (`apps.scan_programs`). Off stops the scan and removes programs from `!`. Off also stops the watcher that picks up new programs.
