//! T2 acceptance tests: built-in registry, Flick defaults, imports. Public API only.

use launcher_core::brand::{DEFAULT_THEME_ID, DEFAULT_THEME_LIGHT_ID};
use launcher_core::theme::color::{parse_hex, ratio};
use launcher_core::theme::derive::MIN_SEL_CONTRAST;
use launcher_core::theme::{Registry, Seed, Source};
use std::collections::HashSet;

fn seed(id: &str, bg: &str) -> Seed {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": format!("Name {id}"),
        "source": "imported",
        "bg": bg,
        "panel": "#11161c",
        "text": "#e6edf3",
        "muted": "#9aa7b4",
        "accent": "#5fa8ff",
    }))
    .expect("seed json")
}

fn builtin() -> Registry {
    Registry::builtin().expect("builtin registry parses both embedded JSONs")
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[test]
fn builtin_has_sixteen_own_themes_plus_every_port() {
    let reg = builtin();
    let own = reg
        .list()
        .iter()
        .filter(|t| t.seed.source == Source::Builtin)
        .count();
    assert_eq!(own, 16);
    // The port count follows ports.json, never a hard-coded number.
    let ports: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../src/theme/seeds/ports.json")).expect("ports json");
    assert_eq!(reg.list().len(), own + ports.len());
}

#[test]
fn every_theme_meets_contrast() {
    for t in builtin().list() {
        let id = &t.seed.id;
        let r = &t.resolved.ratios;
        let roles = &t.resolved.roles;
        let text = parse_hex(&roles.text).expect("text hex");
        let bg = parse_hex(&roles.bg).expect("bg hex");
        let panel = parse_hex(&roles.panel).expect("panel hex");
        assert!(ratio(text, panel) >= 4.5, "{id}: text on panel");
        assert!(ratio(text, bg) >= 4.5, "{id}: text on bg");
        let sel = parse_hex(&roles.sel).expect("sel hex");
        assert!(
            ratio(sel, panel) >= MIN_SEL_CONTRAST,
            "{id}: sel is not distinguishable from panel"
        );
        for (name, v) in [
            ("text", r.text),
            ("muted", r.muted),
            ("accent_text", r.accent_text),
            ("on_accent", r.on_accent),
            ("sel_text", r.sel_text),
            ("muted_bg", r.muted_bg),
            ("muted_sel", r.muted_sel),
        ] {
            assert!(v >= 4.5, "{id}: {name} ratio {v}");
        }
    }
}

#[test]
fn ids_unique_and_kebab_case() {
    let reg = builtin();
    let mut seen = HashSet::new();
    for t in reg.list() {
        assert!(is_kebab(&t.seed.id), "bad id {:?}", t.seed.id);
        assert!(seen.insert(t.seed.id.clone()), "dup id {:?}", t.seed.id);
    }
}

#[test]
fn names_unique_and_non_empty() {
    let reg = builtin();
    let mut seen = HashSet::new();
    for t in reg.list() {
        assert!(
            !t.seed.name.trim().is_empty(),
            "empty name for {}",
            t.seed.id
        );
        assert!(
            seen.insert(t.seed.name.clone()),
            "dup name {:?}",
            t.seed.name
        );
    }
}

#[test]
fn flick_default_dark_roles_are_fixed() {
    // Keep equal to the fallback block in tokens.css.
    let reg = builtin();
    let t = reg.get(DEFAULT_THEME_ID).expect("dark default");
    let r = &t.resolved.roles;
    assert!(t.resolved.dark);
    assert_eq!(r.bg, "#0c0e12");
    assert_eq!(r.panel, "#14171d");
    assert_eq!(r.text, "#e8ebf0");
    assert_eq!(r.muted, "#b6bbc3");
    assert_eq!(r.accent, "#5cc8e8");
    assert_eq!(r.sel, "#1d2a33");
    assert_eq!(r.line, "#262b34");
    assert_eq!(r.surface, "#1b1f27");
}

#[test]
fn flick_default_light_roles_are_fixed() {
    // Keep equal to the fallback block in tokens.css.
    let reg = builtin();
    let t = reg.get(DEFAULT_THEME_LIGHT_ID).expect("light default");
    let r = &t.resolved.roles;
    assert!(!t.resolved.dark);
    assert_eq!(r.bg, "#f4f8fb");
    assert_eq!(r.panel, "#ffffff");
    assert_eq!(r.line, "#d0dee8");
    assert_eq!(r.surface, "#e7eff5");
}

#[test]
fn default_for_returns_the_two_defaults() {
    for reg in [Registry::defaults_only(), builtin()] {
        assert_eq!(
            reg.default_for(true).expect("dark").seed.id,
            DEFAULT_THEME_ID
        );
        assert_eq!(
            reg.default_for(false).expect("light").seed.id,
            DEFAULT_THEME_LIGHT_ID
        );
    }
}

#[test]
fn defaults_only_has_just_the_defaults() {
    let reg = Registry::defaults_only();
    assert_eq!(reg.list().len(), 2);
    assert!(reg.contains(DEFAULT_THEME_ID));
    assert!(reg.contains(DEFAULT_THEME_LIGHT_ID));
    assert!(!reg.contains("nope"));
    assert!(reg.get("nope").is_none());
}

#[test]
fn with_imported_requires_imported_slug_ids() {
    let long_ok = format!("imported:{}", "a".repeat(64));
    let too_long = format!("imported:{}", "a".repeat(65));
    let reg = Registry::defaults_only().with_imported(vec![
        seed("x", "#0b0f14"),
        seed("imported:Bad", "#0b0f14"),
        seed("imported:", "#0b0f14"),
        seed(&too_long, "#0b0f14"),
        seed("imported:a_b", "#0b0f14"),
        seed("imported:x", "#0b0f14"),
        seed(&long_ok, "#0b0f14"),
    ]);
    for dropped in ["x", "imported:Bad", "imported:", &too_long, "imported:a_b"] {
        assert!(!reg.contains(dropped), "{dropped} should be dropped");
    }
    assert!(reg.contains("imported:x"));
    assert!(reg.contains(&long_ok));
    assert_eq!(reg.list().len(), 4);
}

#[test]
fn with_imported_keeps_valid_drops_duplicate_and_invalid() {
    let reg = Registry::defaults_only().with_imported(vec![
        seed("imported:x", "#0b0f14"),
        seed("imported:x", "#101010"),
        seed(DEFAULT_THEME_ID, "#0b0f14"),
        seed("imported:bad", "red"),
    ]);
    assert_eq!(reg.list().len(), 3);
    assert!(reg.contains("imported:x"));
    assert!(!reg.contains("imported:bad"));
    // The first import wins and the default is not overwritten.
    assert_eq!(reg.get("imported:x").expect("x").seed.bg, "#0b0f14");
    assert_eq!(
        reg.get(DEFAULT_THEME_ID)
            .expect("default")
            .resolved
            .roles
            .bg,
        "#0c0e12"
    );
}
