//! sRGB colour math: parsing, WCAG contrast, mixing and contrast lift.

/// An opaque sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// White, a lift pole.
pub const WHITE: Rgb = Rgb(255, 255, 255);
/// Black, a lift pole.
pub const BLACK: Rgb = Rgb(0, 0, 0);

fn nibble(b: u8) -> Option<u8> {
    char::from(b)
        .to_digit(16)
        .and_then(|d| u8::try_from(d).ok())
}

/// Decodes `#rgb` (doubled nibbles), `#rrggbb` or `#rrggbbaa` into 3, 3 or 4 bytes.
fn bytes(s: &str) -> Option<Vec<u8>> {
    let h = s.strip_prefix('#')?.as_bytes();
    match h.len() {
        3 => h.iter().map(|&b| nibble(b).map(|n| n * 17)).collect(),
        6 | 8 => h
            .chunks(2)
            .map(|p| Some(nibble(p[0])? * 16 + nibble(*p.get(1)?)?))
            .collect(),
        _ => None,
    }
}

/// Parses `#rgb` or `#rrggbb` (case-insensitive). Anything else is `None`.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    match bytes(s)?.as_slice() {
        [r, g, b] => Some(Rgb(*r, *g, *b)),
        _ => None,
    }
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`; alpha is 255 when absent. `#rgba` is rejected.
pub fn parse_hex_alpha(s: &str) -> Option<(Rgb, u8)> {
    match bytes(s)?.as_slice() {
        [r, g, b] => Some((Rgb(*r, *g, *b), 255)),
        [r, g, b, a] => Some((Rgb(*r, *g, *b), *a)),
        _ => None,
    }
}

/// Formats as lowercase `#rrggbb`.
pub fn to_hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// WCAG relative luminance (sRGB, 0.04045 break).
pub fn luminance(c: Rgb) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// WCAG contrast ratio, 1.0 to 21.0, symmetric.
pub fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn lerp(a: u8, b: u8, t: f64) -> u8 {
    let v = f64::from(a) + (f64::from(b) - f64::from(a)) * t;
    // Clamped to 0..=255 first, so the cast cannot truncate.
    v.round().clamp(0.0, 255.0) as u8
}

/// Linear mix in sRGB space: `t` 0 gives `a`, 1 gives `b`.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    Rgb(lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

/// Composites `fg` at `alpha` (0..=255) over `bg`.
pub fn blend_over(fg: Rgb, alpha: u8, bg: Rgb) -> Rgb {
    mix(bg, fg, f64::from(alpha) / 255.0)
}

/// Lowest contrast ratio of `c` over all `bgs`.
pub fn min_ratio(c: Rgb, bgs: &[Rgb]) -> f64 {
    bgs.iter()
        .map(|&b| ratio(c, b))
        .fold(f64::INFINITY, f64::min)
}

/// Raises `fg` to `target` contrast on every background in `bgs`, in 4% steps toward the pole
/// (white or black) that contrasts better with them. Returns the pole when `target` is unreachable.
///
/// Correction 1 of the port: THM picks the pole from `lum(bg) < 0.5`, which cannot reach 4.5:1 on
/// mid-tone panels; here the pole is chosen by comparing the actual ratios.
pub fn lift_all(fg: Rgb, bgs: &[Rgb], target: f64) -> Rgb {
    if min_ratio(fg, bgs) >= target {
        return fg;
    }
    let pole = if min_ratio(WHITE, bgs) >= min_ratio(BLACK, bgs) {
        WHITE
    } else {
        BLACK
    };
    (1..=25)
        .map(|i| mix(fg, pole, f64::from(i) * 0.04))
        .find(|&c| min_ratio(c, bgs) >= target)
        .unwrap_or(pole)
}

/// [`lift_all`] against a single background.
pub fn lift(fg: Rgb, bg: Rgb, target: f64) -> Rgb {
    lift_all(fg, &[bg], target)
}
