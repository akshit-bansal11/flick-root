//! Theme commands and the active-theme lookup.
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::theme::{Ratios, Registry as ThemeRegistry, Roles, Source, Theme};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// Role colours as `#rrggbb`, as the UI receives them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolesView {
    bg: String,
    panel: String,
    surface: String,
    line: String,
    text: String,
    muted: String,
    accent: String,
    accent_text: String,
    on_accent: String,
    sel: String,
    ok: String,
    warn: String,
    err: String,
}

impl From<&Roles> for RolesView {
    fn from(r: &Roles) -> Self {
        Self {
            bg: r.bg.clone(),
            panel: r.panel.clone(),
            surface: r.surface.clone(),
            line: r.line.clone(),
            text: r.text.clone(),
            muted: r.muted.clone(),
            accent: r.accent.clone(),
            accent_text: r.accent_text.clone(),
            on_accent: r.on_accent.clone(),
            sel: r.sel.clone(),
            ok: r.ok.clone(),
            warn: r.warn.clone(),
            err: r.err.clone(),
        }
    }
}

/// Contrast ratios of a theme's roles.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatiosView {
    text: f64,
    muted: f64,
    accent_text: f64,
    on_accent: f64,
    sel_text: f64,
    muted_bg: f64,
    muted_sel: f64,
}

impl From<Ratios> for RatiosView {
    fn from(r: Ratios) -> Self {
        Self {
            text: r.text,
            muted: r.muted,
            accent_text: r.accent_text,
            on_accent: r.on_accent,
            sel_text: r.sel_text,
            muted_bg: r.muted_bg,
            muted_sel: r.muted_sel,
        }
    }
}

/// One theme as listed to the UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeView {
    /// Stable id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Builtin, port or imported.
    pub source: Source,
    /// True when the panel is dark.
    pub dark: bool,
    /// Derived role colours.
    pub roles: RolesView,
    /// Contrast ratios of those roles.
    pub ratios: RatiosView,
    /// Colours that were lifted to reach the contrast target.
    pub adjusted: Vec<String>,
    /// Upstream theme URL, for ports.
    pub upstream: Option<String>,
}

impl From<&Theme> for ThemeView {
    fn from(t: &Theme) -> Self {
        Self {
            id: t.seed.id.clone(),
            name: t.seed.name.clone(),
            source: t.seed.source,
            dark: t.resolved.dark,
            roles: (&t.resolved.roles).into(),
            ratios: t.resolved.ratios.into(),
            adjusted: t.resolved.adjusted.clone(),
            upstream: t.seed.upstream.clone(),
        }
    }
}

/// Maps every theme in the registry to its view.
pub fn theme_views(registry: &ThemeRegistry) -> Vec<ThemeView> {
    registry.list().iter().map(ThemeView::from).collect()
}

/// Lists every known theme with its derived roles (resolved values are cached in the registry).
#[tauri::command]
pub async fn list_themes(state: State<'_, Arc<AppState>>) -> Result<Vec<ThemeView>, CommandError> {
    Ok(theme_views(&state.themes.load()))
}

/// The theme currently in force for the configured scheme.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveTheme {
    /// Theme id.
    pub id: String,
    /// True when the panel is dark.
    pub dark: bool,
    /// Derived role colours.
    pub roles: Roles,
}

/// Whether a configured scheme resolves to dark; "system" follows Windows.
pub fn scheme_is_dark(scheme: &str) -> bool {
    match scheme {
        "dark" => true,
        "light" => false,
        #[cfg(windows)]
        _ => win::dwm::system_prefers_dark(),
        #[cfg(not(windows))]
        _ => true,
    }
}

/// Resolves scheme, then the configured id for that scheme, then the registry; an unknown id
/// falls back to the code default. `None` only if the registry lost its defaults.
pub fn active_theme_in(
    registry: &ThemeRegistry,
    dark: bool,
    dark_id: &str,
    light_id: &str,
) -> Option<ActiveTheme> {
    let id = if dark { dark_id } else { light_id };
    let theme = registry.get(id).or_else(|| registry.default_for(dark))?;
    Some(ActiveTheme {
        id: theme.seed.id.clone(),
        dark,
        roles: theme.resolved.roles.clone(),
    })
}

/// The active theme for the live config.
pub fn active_theme(state: &AppState) -> Option<ActiveTheme> {
    let cfg = state.config.load();
    let a = &cfg.appearance;
    active_theme_in(
        &state.themes.load(),
        scheme_is_dark(&a.scheme),
        a.dark_id(),
        a.light_id(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::brand::{DEFAULT_THEME_ID, DEFAULT_THEME_LIGHT_ID};

    fn registry() -> ThemeRegistry {
        ThemeRegistry::builtin().expect("builtin registry")
    }

    #[test]
    fn theme_views_map_every_registry_theme() {
        let reg = registry();
        let views = theme_views(&reg);
        assert_eq!(views.len(), reg.list().len());
        for (v, t) in views.iter().zip(reg.list()) {
            assert_eq!(v.id, t.seed.id);
            assert_eq!(v.name, t.seed.name);
            assert_eq!(v.dark, t.resolved.dark);
        }
        assert!(views.iter().any(|v| v.id == DEFAULT_THEME_ID));
        assert!(views.iter().any(|v| v.id == DEFAULT_THEME_LIGHT_ID));
    }

    #[test]
    fn theme_view_serializes_camel_case_with_lowercase_source() {
        let views = theme_views(&registry());
        let json = serde_json::to_value(&views[0]).expect("serialize");
        for key in [
            "id", "name", "source", "dark", "roles", "ratios", "adjusted", "upstream",
        ] {
            assert!(json.get(key).is_some(), "missing {key} in {json}");
        }
        assert!(json["roles"].get("accentText").is_some(), "{json}");
        assert!(json["roles"].get("onAccent").is_some(), "{json}");
        assert!(json["ratios"].get("onAccent").is_some(), "{json}");
        assert!(json["ratios"].get("mutedSel").is_some(), "{json}");
        let source = json["source"].as_str().expect("source string");
        assert_eq!(source, source.to_lowercase());
        assert!(
            ["builtin", "port", "imported"].contains(&source),
            "{source}"
        );
    }

    #[test]
    fn active_theme_in_picks_slot_by_scheme() {
        let reg = registry();
        let dark = active_theme_in(&reg, true, "midnight", "frost").expect("dark");
        assert_eq!(dark.id, "midnight");
        assert!(dark.dark);
        let light = active_theme_in(&reg, false, "midnight", "frost").expect("light");
        assert_eq!(light.id, "frost");
        assert!(!light.dark);
    }

    #[test]
    fn active_theme_in_falls_back_to_default_for_unknown_id() {
        let reg = registry();
        let dark = active_theme_in(&reg, true, "nope", "nope").expect("dark");
        assert_eq!(dark.id, DEFAULT_THEME_ID);
        let light = active_theme_in(&reg, false, "nope", "nope").expect("light");
        assert_eq!(light.id, DEFAULT_THEME_LIGHT_ID);
    }
}
