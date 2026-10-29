# Thinking modes

In Ask (`?`), each enabled provider and model pair is a thinking mode.

## How to use it

- `Ctrl+T` switches to the next model and wraps around. The chosen one answers first, then the usual provider fallback applies if it fails. It is remembered (`modes.query.thinking_mode`).
- `Shift+Tab` cycles the effort: auto, low, medium, high, then xhigh and max where the model lists them, then auto again. It is remembered (`modes.query.effort`).
- Auto omits the effort parameter, so the model's default applies. The effort row is hidden when the model lists no levels.
- Only the first provider in a fallback chain gets the chosen effort.
- Plain `Tab` and the previous-model shortcut no longer switch anything.

## Adding models

A provider's configured model is one mode. Add more in Settings, AI providers, "Extra models" (`providers.<id>.models`); each becomes its own mode. If the remembered mode is gone, the first mode is used.

Models and efforts are fetched from the provider when you save its key (Anthropic: active models with their effort levels; OpenAI-compatible: up to 20 model ids, with low, medium and high when supported). Settings, AI providers, "refresh models" fetches them again. A refresh replaces the "Extra models" list.

Nothing is on by default: with no enabled provider there are no modes and Ask shows "Connect a model".
