# Programs

`!` lists your installed apps and, below them, programs Flick finds by scanning every fixed drive.

## How to use it

Type `!` first in the empty bar, then part of a name. Installed apps rank first. A program row shows "Program" and its folder. Enter opens it. `Ctrl+Shift+Enter` runs it as admin, only for `.exe` and `.msi` files.

## What is scanned

- Extensions: exe, msi, msix, msixbundle, appx, appxbundle, apk, xapk, aab, bat, cmd, ps1, lnk, url, appref-ms, jar, appinstaller, app, pkg, dmg, appimage, deb, rpm, flatpak, flatpakref, snap, run, sh.
- Skipped folders: windows, $recycle.bin, system volume information, $winreagent, $sysreset, recovery, config.msi, winsxs, node_modules, .git, target, __pycache__, .cache, temp, tmp, recent, onedrivetemp, and start menu (its shortcuts already appear as apps).
- At most 50,000 programs are kept; the walk stops there.
- The scan runs at startup. There is no watcher, so a program installed later appears after the next start.
- Programs match in their own index, off the keystroke thread.

## Notes

- Icons are loaded only for local `.exe` files. Other types, and files on network paths, get a generic icon, to avoid reaching out to a network share (NTLM credential exposure).
- Mac and Linux package types (dmg, deb, rpm and so on) open with whatever Windows associates with them; Flick does not install them.

## Turn it off

Settings, General, "Find programs on all drives" (`apps.scan_programs`). Off stops the scan and removes programs from `!`.
