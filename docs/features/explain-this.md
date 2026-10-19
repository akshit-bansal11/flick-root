# Explain this

A second hotkey captures the text you have selected in another app and opens Flick in Query mode with it attached. You see exactly what will be sent before anything is sent.

## How to use it

1. Select text in any app.
2. Press the Explain hotkey. Default: `Alt+Shift+Space` (changeable in Settings, General).
3. Flick opens in Query mode with a preview of the captured text and the app it came from. Type a question, or send it as is; with no question typed, the question is "Explain this".
4. Press Enter to send it to your Query provider. Nothing is sent before you press Enter.

The hotkey does nothing if the launcher is already showing (there is no foreground selection to read) or if Query mode is off. Explain this is a feature module: turn it off in Settings, Features and the hotkey is released.

## How the text is captured

First through Windows UI Automation (no clipboard). If the app does not support that, Flick sends Ctrl+C and reads the clipboard, then restores what the clipboard held before.

## Options

The hotkey (General). The config also has `max_chars = 8000` under the explain feature; the preview, and what is sent, is the first 2,000 characters of the selection.

## Privacy

- Nothing is captured until you press the hotkey, and nothing is sent until you press Enter.
- Text that looks like a secret or API key is **blocked**: it is not previewed and not sent, and Flick says why. The check covers the whole selection, and runs again when you press Enter.
- When you do send, the text goes to the Query provider you chose and nowhere else. Flick has no default provider; use a local one if you want it to stay on your machine.

## Limits

- Needs Query mode on and at least one enabled provider.
- Apps that expose neither UI Automation text nor Ctrl+C copy give an empty capture.
- The secret check is a heuristic; it can miss a secret or block harmless text.
