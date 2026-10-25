//! Role derivation: a [`Seed`] becomes the thirteen roles the UI paints with, every text role
//! contrast-checked. Port of THM `derive` with three corrections:
//! 1. `lift` chooses its pole by comparing real ratios (see `color::lift_all`).
//! 2. `on_accent` also tries `#000000`; `#111111` alone leaves a 4.35:1 worst case.
//! 3. `muted` and `accent_text` are lifted against panel, bg and sel, and `text` against panel
//!    and bg, because the tab strip, status line and nav sit on bg and tags sit on sel.

use super::color::{
    BLACK, Rgb, WHITE, lift_all, luminance, min_ratio, mix, parse_hex, ratio, to_hex,
};
use super::{Seed, ThemeError};

const TARGET: f64 = 4.5;

/// Least contrast ratio a selected-row colour needs against the panel to be visible. A seed `sel`
/// below it (including one equal to the panel, ratio 1.0) is ignored and `sel` is derived instead.
/// 1.1 is the smallest step that is still a visible tint; the derived mix clears it for every
/// built-in theme.
pub const MIN_SEL_CONTRAST: f64 = 1.1;
const DARK_STATUS: [Rgb; 3] = [
    Rgb(0x4c, 0xc3, 0x8a),
    Rgb(0xea, 0xb8, 0x5c),
    Rgb(0xf6, 0xad, 0xa6),
];
const LIGHT_STATUS: [Rgb; 3] = [
    Rgb(0x16, 0x58, 0x3b),
    Rgb(0x6c, 0x46, 0x00),
    Rgb(0x8c, 0x2c, 0x22),
];

/// Final colours as lowercase `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roles {
    /// Window background.
    pub bg: String,
    /// Launcher panel.
    pub panel: String,
    /// Raised surface.
    pub surface: String,
    /// Border.
    pub line: String,
    /// Body text.
    pub text: String,
    /// Secondary text.
    pub muted: String,
    /// Accent fill.
    pub accent: String,
    /// Accent used as text.
    pub accent_text: String,
    /// Text on an accent fill.
    pub on_accent: String,
    /// Selected row.
    pub sel: String,
    /// Success.
    pub ok: String,
    /// Warning.
    pub warn: String,
    /// Error.
    pub err: String,
}

/// Contrast ratios of the final roles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ratios {
    /// Text, worst of panel and bg.
    pub text: f64,
    /// Muted on panel.
    pub muted: f64,
    /// Accent text on panel.
    pub accent_text: f64,
    /// On-accent text on accent.
    pub on_accent: f64,
    /// Text on sel.
    pub sel_text: f64,
    /// Muted on bg.
    pub muted_bg: f64,
    /// Muted on sel.
    pub muted_sel: f64,
}

/// A resolved theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// True when the panel is dark.
    pub dark: bool,
    /// Derived roles.
    pub roles: Roles,
    /// Contrast ratios of those roles.
    pub ratios: Ratios,
    /// One line per colour that was lifted, like `muted 3.1->4.5`.
    pub adjusted: Vec<String>,
}

fn hex(field: &'static str, s: &str) -> Result<Rgb, ThemeError> {
    parse_hex(s).ok_or(ThemeError::BadColour { field })
}

fn opt(field: &'static str, s: Option<&String>) -> Result<Option<Rgb>, ThemeError> {
    s.map(|v| hex(field, v)).transpose()
}

/// Lifts `fg` against `bgs`, noting the change in `adjusted`.
fn lifted(name: &str, fg: Rgb, bgs: &[Rgb], adjusted: &mut Vec<String>) -> Rgb {
    let out = lift_all(fg, bgs, TARGET);
    if out != fg {
        adjusted.push(format!(
            "{name} {:.1}->{:.1}",
            min_ratio(fg, bgs),
            min_ratio(out, bgs)
        ));
    }
    out
}

/// Derives every role from `seed`, lifting text colours to 4.5:1.
pub fn resolve(seed: &Seed) -> Result<Resolved, ThemeError> {
    let bg = hex("bg", &seed.bg)?;
    let panel = hex("panel", &seed.panel)?;
    let accent = hex("accent", &seed.accent)?;
    let dark = luminance(panel) < 0.5;
    let mut adjusted = Vec::new();

    let text = lifted(
        "text",
        hex("text", &seed.text)?,
        &[panel, bg],
        &mut adjusted,
    );

    // A seed `sel` too close to the panel would hide the selected row, so it counts as absent.
    let given = opt("sel", seed.sel.as_ref())?.filter(|&c| ratio(c, panel) >= MIN_SEL_CONTRAST);
    let mut sel = given.unwrap_or_else(|| mix(panel, accent, if dark { 0.22 } else { 0.16 }));
    for _ in 0..20 {
        if ratio(text, sel) >= TARGET {
            break;
        }
        sel = mix(sel, panel, 0.15);
    }
    if ratio(text, sel) < TARGET {
        sel = panel;
    }

    let on = [panel, bg, sel];
    let muted = lifted("muted", hex("muted", &seed.muted)?, &on, &mut adjusted);
    let accent_text = lifted("accent_text", accent, &on, &mut adjusted);

    let line = opt("line", seed.line.as_ref())?.unwrap_or_else(|| mix(panel, text, 0.14));
    let surface = opt("surface", seed.surface.as_ref())?
        .unwrap_or_else(|| mix(panel, text, if dark { 0.04 } else { 0.07 }));

    let on_accent = [bg, panel, WHITE, Rgb(0x11, 0x11, 0x11), BLACK]
        .into_iter()
        .fold(bg, |best, c| {
            if ratio(c, accent) > ratio(best, accent) {
                c
            } else {
                best
            }
        });

    let status = if dark { DARK_STATUS } else { LIGHT_STATUS };
    let [ok, warn, err] = status.map(|c| lift_all(c, &[panel, bg, sel], TARGET));

    let ratios = Ratios {
        text: min_ratio(text, &[panel, bg]),
        muted: ratio(muted, panel),
        accent_text: ratio(accent_text, panel),
        on_accent: ratio(on_accent, accent),
        sel_text: ratio(text, sel),
        muted_bg: ratio(muted, bg),
        muted_sel: ratio(muted, sel),
    };
    let checks = [
        ("text", ratios.text, "panel or bg"),
        ("muted", ratios.muted, "panel"),
        ("accent_text", ratios.accent_text, "panel"),
        ("on_accent", ratios.on_accent, "accent"),
        ("text", ratios.sel_text, "sel"),
        ("muted", ratios.muted_bg, "bg"),
        ("muted", ratios.muted_sel, "sel"),
    ];
    if let Some((role, r, on)) = checks.into_iter().find(|c| c.1 < TARGET) {
        return Err(ThemeError::Unreadable {
            worst: format!("{role} {r:.2} on {on}"),
        });
    }

    Ok(Resolved {
        dark,
        ratios,
        roles: Roles {
            bg: to_hex(bg),
            panel: to_hex(panel),
            surface: to_hex(surface),
            line: to_hex(line),
            text: to_hex(text),
            muted: to_hex(muted),
            accent: to_hex(accent),
            accent_text: to_hex(accent_text),
            on_accent: to_hex(on_accent),
            sel: to_hex(sel),
            ok: to_hex(ok),
            warn: to_hex(warn),
            err: to_hex(err),
        },
        adjusted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_with_sel(sel: &str) -> Seed {
        serde_json::from_value(serde_json::json!({
            "id": "t",
            "name": "T",
            "source": "imported",
            "bg": "#0b0f14",
            "panel": "#11161c",
            "text": "#e6edf3",
            "muted": "#9aa7b4",
            "accent": "#5fa8ff",
            "sel": sel,
        }))
        .expect("seed json")
    }

    #[test]
    fn sel_equal_to_panel_is_derived_instead() {
        let r = resolve(&seed_with_sel("#11161c")).expect("resolves");
        let (sel, panel) = (
            parse_hex(&r.roles.sel).expect("sel hex"),
            parse_hex(&r.roles.panel).expect("panel hex"),
        );
        assert_ne!(r.roles.sel, r.roles.panel);
        assert!(ratio(sel, panel) >= MIN_SEL_CONTRAST);
        assert!(r.ratios.sel_text >= TARGET);
    }

    #[test]
    fn distinct_sel_is_kept() {
        let r = resolve(&seed_with_sel("#1d2a33")).expect("resolves");
        assert_eq!(r.roles.sel, "#1d2a33");
    }
}
