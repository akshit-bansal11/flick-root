# Appearance

The launcher is a rounded panel with an acrylic backdrop. Its height follows the results. When Windows transparency effects are off, the panel is opaque.

## Settings, Appearance

- **Theme:** Glass, Frost, Midnight, Aurora, Ember or Solid. Default Glass.
- **Glass strength:** tint opacity, 80 to 95 percent. Default 90. It saves when you release the slider. The floor exists so text stays readable over a white or black desktop.
- **Scheme:** System, Dark or Light. Default System.

## Contrast

`apps/desktop/src/tokens.contrast.test.ts` reads `tokens.css` and checks every theme and scheme: text roles at 4.5:1 and control edges at 3:1 against the panel tint over a white and a black backdrop at the glass floor, and over the opaque panel.

The choice is applied to the launcher every time it is shown. It is stored under `[appearance]` in `config.toml`.
