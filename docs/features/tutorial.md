# Welcome and tour

## Welcome screen

On first launch, setup opens with a welcome screen: the animated logo, the five prefixes (`/`, `>`, `=`, `?`, `@`) and "Take a 2-minute interactive tour?" with two buttons.

- **Take the tour** starts the tour.
- **Not now** shows a note that you can take the tour any time from Settings > General > Take the tour, then Continue.

The launcher itself never shows the tour.

## The tour

The tour runs in its own window on the real launcher UI, fed with sample data. Nothing is opened or copied for real, and nothing is sent to a model.

Steps, in order: Open the bar (your hotkey), Type anything (search with no prefix), `/` Open, `>` Commands, `=` Calc (copy the answer), `?` Ask (switch model, ask), Explain this (select the sample text, press the Explain hotkey), Getting around, Themes (the bar restyles live), and a closing step. A step checks itself off when you do what it asks and moves on after a short pause.

- **Show me** types the step's example for you (or presses the hotkey, selects the text, or picks a theme).
- `PageDown` / `PageUp`: next / previous step.

While the tour runs, the global hotkeys are paused so the demo can see the keys. They resume when the tour ends or Settings closes.

Settings > General > "Take the tour" starts it again.
