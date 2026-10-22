# Explain this

A second hotkey captures the text you have selected in another app and opens Flick in Ask (`?`) with it attached. You see exactly what will be sent before anything is sent.

## How to use it

1. Select text in any app.
2. Press the Explain hotkey. Default: `Alt+Shift+Space` (changeable in Settings, General).
3. Flick opens in Ask with a preview of the captured text and the app it came from. Type a question, or send it as is; with no question typed, the question is "Explain this".
4. Press Enter to send it to your Ask provider. Nothing is sent before you press Enter.

The hotkey does nothing if the launcher is already showing (there is no foreground selection to read) or if Ask is off. Explain this is a feature module: turn it off in Settings, Prefixes and the hotkey is released.

## How the text is captured

First through Windows UI Automation (no clipboard). If the app does not support that, Flick sends Ctrl+C and reads the clipboard, then restores the text the clipboard held before. Only text is restored: images, files and other clipboard formats are not preserved.

Flick never sends Ctrl+C to terminals or to apps that host one (VS Code, Cursor, JetBrains IDEs, Windows Terminal, and similar), because Ctrl+C there can interrupt a running process. In those apps, and when the foreground app cannot be identified, it relies on UI Automation only.

## Options

The hotkey (General). The preview, and what is sent, is the first 2,000 characters of the selection (fixed; there is no length option).

## Privacy

- Nothing is captured until you press the hotkey, and nothing is sent until you press Enter.
- Text that looks like a secret or API key is **blocked**: it is not previewed and not sent, and Flick says why. The check covers the whole selection, and runs again when you press Enter.
- When you do send, the text goes to the Ask provider you chose and nowhere else. Flick has no default provider; use a local one if you want it to stay on your machine.

## Limits

- Needs Ask on and at least one enabled provider.
- Apps that expose neither UI Automation text nor Ctrl+C copy give an empty capture.
- The secret check is a heuristic; it can miss a secret or block harmless text.
