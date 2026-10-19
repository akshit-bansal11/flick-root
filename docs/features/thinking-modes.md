# Thinking modes

In Ask (`?`), each enabled provider and model pair is a thinking mode.

## How to use it

- `Tab` / `Shift+Tab` switch to the next or previous mode. The chosen one answers first, then the usual provider fallback applies if it fails. It is remembered (`modes.query.thinking_mode`).
- `Ctrl+T` toggles Fast and Think for the answer.
- `Ctrl+P` (next provider) no longer exists; Tab replaces it.

## Adding models

A provider's configured model is one mode. Add more in Settings, AI providers, "Extra models" (`providers.<id>.models`); each becomes its own mode. If the remembered mode is gone, the first mode is used.

Nothing is on by default: with no enabled provider there are no modes and Ask shows "Connect a model".
