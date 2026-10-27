//! Acceptance tests: colour math and role derivation, public API only.

use launcher_core::theme::color::{
    BLACK, WHITE, blend_over, lift, lift_all, luminance, min_ratio, mix, parse_hex,
    parse_hex_alpha, ratio, to_hex,
};
use launcher_core::theme::{Ratios, Rgb, Seed, ThemeError, resolve};

fn seed(bg: &str, panel: &str, text: &str, muted: &str, accent: &str) -> Seed {
    serde_json::from_value(serde_json::json!({
        "id": "t",
        "name": "T",
        "source": "builtin",
        "bg": bg,
        "panel": panel,
        "text": text,
        "muted": muted,
        "accent": accent,
    }))
    .expect("seed json")
}

fn all_ratios(r: &Ratios) -> [f64; 7] {
    [
        r.text,
        r.muted,
        r.accent_text,
        r.on_accent,
        r.sel_text,
        r.muted_bg,
        r.muted_sel,
    ]
}

fn assert_all_pass(s: &Seed) {
    let res = resolve(s).expect("resolve");
    for v in all_ratios(&res.ratios) {
        assert!(v >= 4.5, "ratio {v} < 4.5 in {:?}", res.ratios);
    }
}

#[test]
fn parse_hex_good() {
    assert_eq!(parse_hex("#fff"), Some(WHITE));
    assert_eq!(parse_hex("#FFF"), Some(WHITE));
    assert_eq!(parse_hex("#000000"), Some(BLACK));
    assert_eq!(parse_hex("#AbCdEf"), Some(Rgb(0xab, 0xcd, 0xef)));
    assert_eq!(parse_hex("#1a2"), Some(Rgb(0x11, 0xaa, 0x22)));
}

#[test]
fn parse_hex_bad() {
    for s in [
        "#abcd",
        " #fff",
        "#fff ",
        "#GGG",
        "#12345",
        "#1234567",
        "#aabbccdd",
        "fff",
        "",
        "#",
        "red",
        "rgb(1,2,3)",
    ] {
        assert_eq!(parse_hex(s), None, "{s:?} should be rejected");
    }
}

#[test]
fn parse_hex_alpha_accepts_rrggbbaa() {
    assert_eq!(
        parse_hex_alpha("#11223380"),
        Some((Rgb(0x11, 0x22, 0x33), 0x80))
    );
    assert_eq!(parse_hex_alpha("#fff"), Some((WHITE, 255)));
    assert_eq!(parse_hex_alpha("#ffffff"), Some((WHITE, 255)));
}

#[test]
fn parse_hex_alpha_rejects() {
    for s in [
        "#abcd",
        " #fff",
        "#GGG",
        "red",
        "rgb(1,2,3)",
        "rgba(1,2,3,1)",
        "",
    ] {
        assert_eq!(parse_hex_alpha(s), None, "{s:?} should be rejected");
    }
}

#[test]
fn to_hex_lowercase_rrggbb() {
    assert_eq!(to_hex(Rgb(0xAB, 0x0C, 0xEF)), "#ab0cef");
    assert_eq!(to_hex(BLACK), "#000000");
    assert_eq!(to_hex(WHITE), "#ffffff");
}

#[test]
fn wcag_anchors() {
    assert!((luminance(WHITE) - 1.0).abs() < 1e-9);
    assert!(luminance(BLACK).abs() < 1e-9);
    assert!((ratio(WHITE, BLACK) - 21.0).abs() < 1e-9);
    assert!((ratio(BLACK, WHITE) - 21.0).abs() < 1e-9);
    assert!((ratio(WHITE, WHITE) - 1.0).abs() < 1e-9);
}

#[test]
fn mix_endpoints() {
    let a = Rgb(10, 20, 30);
    let b = Rgb(200, 150, 100);
    assert_eq!(mix(a, b, 0.0), a);
    assert_eq!(mix(a, b, 1.0), b);
}

#[test]
fn blend_over_known_values() {
    let bg = Rgb(10, 20, 30);
    assert_eq!(blend_over(WHITE, 0, bg), bg);
    assert_eq!(blend_over(WHITE, 255, bg), WHITE);
    assert_eq!(blend_over(Rgb(255, 0, 0), 128, BLACK), Rgb(128, 0, 0));
}

#[test]
fn lift_toward_black_on_light_bg() {
    let fg = Rgb(0x99, 0x99, 0x99);
    let out = lift(fg, WHITE, 4.5);
    assert!(ratio(out, WHITE) >= 4.5);
    assert!(out.0 < fg.0 && out.1 < fg.1 && out.2 < fg.2);
}

#[test]
fn lift_toward_white_on_dark_bg() {
    let fg = Rgb(0x44, 0x44, 0x44);
    let out = lift(fg, BLACK, 4.5);
    assert!(ratio(out, BLACK) >= 4.5);
    assert!(out.0 > fg.0 && out.1 > fg.1 && out.2 > fg.2);
}

#[test]
fn lift_leaves_passing_colour_unchanged() {
    assert_eq!(lift(WHITE, BLACK, 4.5), WHITE);
    assert_eq!(lift(BLACK, WHITE, 4.5), BLACK);
    let c = Rgb(0x20, 0x30, 0x40);
    assert!(ratio(c, WHITE) >= 4.5);
    assert_eq!(lift(c, WHITE, 4.5), c);
}

#[test]
fn lift_reaches_target_on_mid_tone_bg() {
    let bg = Rgb(0x77, 0x77, 0x77);
    for fg in [
        Rgb(0x77, 0x77, 0x77),
        Rgb(0x80, 0x80, 0x80),
        Rgb(0x6a, 0x6a, 0x6a),
    ] {
        let out = lift(fg, bg, 4.5);
        assert!(ratio(out, bg) >= 4.5, "{fg:?} -> {out:?}");
    }
}

#[test]
fn lift_unreachable_returns_a_pole() {
    let bg = Rgb(0x77, 0x77, 0x77);
    let out = lift(Rgb(0x78, 0x78, 0x78), bg, 21.0);
    assert!(out == WHITE || out == BLACK, "{out:?}");
}

#[test]
fn lift_all_and_min_ratio() {
    let bgs = [
        Rgb(0x10, 0x10, 0x10),
        Rgb(0x30, 0x30, 0x30),
        Rgb(0x50, 0x50, 0x50),
    ];
    let out = lift_all(Rgb(0x60, 0x60, 0x60), &bgs, 4.5);
    assert!(min_ratio(out, &bgs) >= 4.5);
    let lo = ratio(out, bgs[0])
        .min(ratio(out, bgs[1]))
        .min(ratio(out, bgs[2]));
    assert!((min_ratio(out, &bgs) - lo).abs() < 1e-9);
}

#[test]
fn resolve_dark_seed() {
    let s = seed("#0b0f14", "#11161c", "#e6edf3", "#9aa7b4", "#5fa8ff");
    assert_all_pass(&s);
    assert!(resolve(&s).expect("resolve").dark);
}

#[test]
fn resolve_light_seed() {
    let s = seed("#f4f1ea", "#ffffff", "#1c1c1c", "#5a5a5a", "#1a6fd6");
    assert_all_pass(&s);
    assert!(!resolve(&s).expect("resolve").dark);
}

#[test]
fn resolve_mid_tone_seed() {
    // bg = panel = #777777: black reaches 4.68, so every role is reachable.
    let s = seed("#777777", "#777777", "#808080", "#7a7a7a", "#7f7f90");
    assert_all_pass(&s);
}

#[test]
fn resolve_rejects_unreachable_seed() {
    // No colour reaches 4.5 on both #707070 and #777777.
    let s = seed("#707070", "#777777", "#808080", "#7a7a7a", "#7f7f90");
    let err = resolve(&s).expect_err("unreachable");
    assert!(matches!(err, ThemeError::Unreadable { .. }), "{err:?}");
    assert!(err.to_string().chars().any(|c| c.is_ascii_digit()), "{err}");
}

#[test]
fn resolve_low_contrast_seed_is_adjusted() {
    let s = seed("#0b0f14", "#11161c", "#e6edf3", "#1f2630", "#5fa8ff");
    let res = resolve(&s).expect("resolve");
    assert!(!res.adjusted.is_empty());
    assert!(res.ratios.muted >= 4.5);
}

#[test]
fn on_accent_passes_across_accent_sweep() {
    for i in 0..60u32 {
        let r = (i * 37) % 256;
        let g = (i * 91) % 256;
        let b = (i * 173) % 256;
        let accent = format!("#{r:02x}{g:02x}{b:02x}");
        for (bg, panel, text, muted) in [
            ("#0b0f14", "#11161c", "#e6edf3", "#9aa7b4"),
            ("#f4f1ea", "#ffffff", "#1c1c1c", "#5a5a5a"),
        ] {
            match resolve(&seed(bg, panel, text, muted, &accent)) {
                Ok(res) => assert!(
                    res.ratios.on_accent >= 4.5,
                    "accent {accent}: on_accent {}",
                    res.ratios.on_accent
                ),
                Err(ThemeError::Unreadable { .. }) => {}
                Err(e) => panic!("accent {accent}: {e}"),
            }
        }
    }
    // Mid-grey accents sit at the luminance where #111111 alone gives about 4.35:1.
    for accent in ["#8a8a8a", "#777777", "#757575", "#6e6e6e", "#808080"] {
        let res =
            resolve(&seed("#0b0f14", "#11161c", "#e6edf3", "#9aa7b4", accent)).expect("resolve");
        assert!(res.ratios.on_accent >= 4.5, "accent {accent}");
    }
}

#[test]
fn adjusted_empty_for_passing_seed() {
    let s = seed("#0b0f14", "#11161c", "#e6edf3", "#b0bcc8", "#8fc1ff");
    let res = resolve(&s).expect("resolve");
    assert!(res.adjusted.is_empty(), "{:?}", res.adjusted);
}

#[test]
fn resolve_rejects_bad_colour() {
    assert!(resolve(&seed("red", "#11161c", "#e6edf3", "#9aa7b4", "#5fa8ff")).is_err());
}
