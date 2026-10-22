# Ports and processes

See what is holding a port, search running processes, and end one, all after typing `>` (Commands).

## How to use it

| Type | Result |
| --- | --- |
| `port` | Every listening process. |
| `port 3000` | The process holding port 3000. |
| `proc node` | Processes whose name contains `node` (case-insensitive), largest memory first. |

Press Enter on the `port ...` or `proc ...` row to list processes (up to 50). Each row shows the name and PID, the connections (for example `TCP 127.0.0.1:3000 LISTEN`), memory, and uptime, and optionally the first 80 characters of the command line.

### Ending a process

1. Use the arrow keys to move onto a process row.
2. Press `Delete`. Before you arrow, Delete just edits the text, so you cannot end a process by accident while typing.
3. Press `Enter` within 3 seconds to confirm. The confirmation is single-use and expires after 3 seconds.

If Windows refuses, Flick says the process may need admin rights.

## Options

Settings, Features, Ports and processes: **Show command line** (default on).

## Privacy

Everything is read locally from Windows. Nothing is sent anywhere. The feature runs no background task; the OS is read only when you press Enter on a row.

## Limits

- At most 50 rows per lookup.
- Ending a process you do not own needs admin rights; Flick does not elevate for you.
- `port` accepts a number from 0 to 65535; anything else is not recognised.
