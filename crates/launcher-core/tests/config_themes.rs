//! Acceptance tests: dark_theme / light_theme and legacy theme migration. Public API only.

use launcher_core::brand::{DEFAULT_THEME_ID, DEFAULT_THEME_LIGHT_ID};
use launcher_core::config::{Appearance, Config};

fn load(toml_text: &str) -> Config {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, toml_text).expect("write");
    Config::load(&path).expect("load")
}

/// Lookup over the two defaults plus `imported:x` (dark) and `imported:l` (light).
fn lookup(id: &str) -> Option<bool> {
    match id {
        DEFAULT_THEME_ID | "imported:x" | "midnight" => Some(true),
        DEFAULT_THEME_LIGHT_ID | "imported:l" | "frost" => Some(false),
        _ => None,
    }
}

#[test]
fn old_general_theme_light_still_loads() {
    let c = load("[general]\ntheme = \"light\"\n");
    assert_eq!(c.appearance.scheme, "light");
    assert_eq!(c.appearance.dark_theme, None);
    assert_eq!(c.appearance.light_theme, None);
    assert_eq!(c.appearance.dark_id(), DEFAULT_THEME_ID);
    assert_eq!(c.appearance.light_id(), DEFAULT_THEME_LIGHT_ID);
}

#[test]
fn legacy_frost_becomes_light_theme_and_light_scheme() {
    let c = load("[appearance]\ntheme = \"frost\"\nscheme = \"system\"\n");
    assert_eq!(c.appearance.light_theme.as_deref(), Some("frost"));
    assert_eq!(c.appearance.dark_theme, None);
    assert_eq!(c.appearance.scheme, "light");
    assert_eq!(c.appearance.light_id(), "frost");
    assert_eq!(c.appearance.dark_id(), DEFAULT_THEME_ID);
}

#[test]
fn legacy_dark_themes_become_dark_theme_and_dark_scheme() {
    for name in ["midnight", "aurora", "ember"] {
        let c = load(&format!(
            "[appearance]\ntheme = \"{name}\"\nscheme = \"system\"\n"
        ));
        assert_eq!(c.appearance.dark_theme.as_deref(), Some(name));
        assert_eq!(c.appearance.light_theme, None);
        assert_eq!(c.appearance.scheme, "dark", "{name}");
    }
}

#[test]
fn legacy_glass_and_solid_change_nothing() {
    for name in ["glass", "solid"] {
        let c = load(&format!(
            "[appearance]\ntheme = \"{name}\"\nscheme = \"system\"\n"
        ));
        assert_eq!(c.appearance.dark_theme, None, "{name}");
        assert_eq!(c.appearance.light_theme, None, "{name}");
        assert_eq!(c.appearance.scheme, "system", "{name}");
        assert_eq!(c.appearance.dark_id(), DEFAULT_THEME_ID);
        assert_eq!(c.appearance.light_id(), DEFAULT_THEME_LIGHT_ID);
    }
}

#[test]
fn round_trip_keeps_both_options() {
    let mut c = Config::default();
    c.appearance.dark_theme = Some("imported:x".into());
    c.appearance.light_theme = Some("frost".into());
    let back: Config = toml::from_str(&toml::to_string(&c).expect("ser")).expect("de");
    assert_eq!(back.appearance.dark_theme.as_deref(), Some("imported:x"));
    assert_eq!(back.appearance.light_theme.as_deref(), Some("frost"));
}

#[test]
fn none_options_are_omitted_when_saved() {
    let text = toml::to_string(&Config::default()).expect("ser");
    assert!(!text.contains("dark_theme"), "{text}");
    assert!(!text.contains("light_theme"), "{text}");
}

#[test]
fn migration_is_idempotent() {
    let mut a = Appearance {
        legacy_theme: Some("midnight".into()),
        ..Appearance::default()
    };
    a.migrate_legacy();
    assert_eq!(a.legacy_theme, None, "consumed");
    assert_eq!(a.dark_theme.as_deref(), Some("midnight"));
    let once = a.clone();
    a.migrate_legacy();
    assert_eq!(a, once);

    // Once an id is chosen, a stale legacy `theme` never overrides it.
    let mut b = Appearance {
        legacy_theme: Some("frost".into()),
        dark_theme: Some("imported:x".into()),
        ..Appearance::default()
    };
    b.migrate_legacy();
    assert_eq!(b.light_theme, None);
    assert_eq!(b.dark_theme.as_deref(), Some("imported:x"));
    assert_eq!(b.scheme, "system");
}

/// True when some line of `text` starts with `key` followed by ` =`.
fn has_key(text: &str, key: &str) -> bool {
    text.lines().any(|l| {
        l.trim_start()
            .strip_prefix(key)
            .is_some_and(|r| r.trim_start().starts_with('='))
    })
}

#[test]
fn saving_never_writes_theme_or_glass() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    let c = load("[appearance]\ntheme = \"frost\"\nglass = 85\n");
    assert_eq!(c.appearance.light_theme.as_deref(), Some("frost"));
    c.save(&path).expect("save");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(!has_key(&text, "theme"), "{text}");
    assert!(!has_key(&text, "glass"), "{text}");
    assert!(has_key(&text, "light_theme"), "{text}");
    // Reloading the saved file is stable.
    assert_eq!(
        Config::load(&path).expect("reload").appearance,
        c.appearance
    );
}

#[test]
fn old_glass_key_loads_and_is_dropped_on_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.toml");
    let c = load("[appearance]\nglass = 85\n");
    assert_eq!(c.appearance.scheme, "system");
    c.save(&path).expect("save");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(!text.contains("glass"), "{text}");
}

#[test]
fn validate_accepts_imported_dark_in_dark_slot() {
    let a = Appearance {
        dark_theme: Some("imported:x".into()),
        ..Appearance::default()
    };
    assert_eq!(a.validate_themes(&lookup), Ok(()));
    assert_eq!(Appearance::default().validate_themes(&lookup), Ok(()));
}

#[test]
fn validate_rejects_light_theme_in_dark_slot_and_vice_versa() {
    let dark_slot = Appearance {
        dark_theme: Some("frost".into()),
        ..Appearance::default()
    };
    assert!(dark_slot.validate_themes(&lookup).is_err());
    let light_slot = Appearance {
        light_theme: Some("midnight".into()),
        ..Appearance::default()
    };
    assert!(light_slot.validate_themes(&lookup).is_err());
}

#[test]
fn validate_rejects_unknown_id_without_echoing_hostile_text() {
    let a = Appearance {
        dark_theme: Some("x\"<b>".into()),
        ..Appearance::default()
    };
    let err = a.validate_themes(&lookup).expect_err("unknown id");
    for bad in ['"', '<', '>'] {
        assert!(!err.contains(bad), "{bad:?} echoed in {err:?}");
    }
    let plain = Appearance {
        light_theme: Some("nope".into()),
        ..Appearance::default()
    };
    assert!(plain.validate_themes(&lookup).is_err());
}
