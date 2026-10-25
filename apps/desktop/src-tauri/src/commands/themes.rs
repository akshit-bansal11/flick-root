//! Theme commands, VS Code theme import, and the active-theme lookup.
use super::blocking;
use crate::error::CommandError;
use crate::state::AppState;
use launcher_core::theme::vscode::{self, MAX_BYTES};
use launcher_core::theme::{
    MappingRow, Ratios, Registry as ThemeRegistry, Roles, Seed, Source, Theme, ThemeError,
    VsImport, resolve,
};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Most files read from the imported-themes folder.
const MAX_IMPORTED: usize = 100;
/// Largest imported-theme file read back from disk, in bytes.
const MAX_SEED_BYTES: u64 = 8 * 1024;
/// Longest display name accepted from a saved theme file, in characters.
const MAX_SAVED_NAME: usize = 100;

/// Role colours as `#rrggbb`, as the UI receives them.
#[derive(Debug, Clone, PartialEq, Serialize)]
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

/// One row of the import mapping table.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingRowView {
    role: String,
    key: String,
    hex: String,
    used: bool,
}

impl From<&MappingRow> for MappingRowView {
    fn from(r: &MappingRow) -> Self {
        Self {
            role: r.role.to_owned(),
            key: r.key.to_owned(),
            hex: r.hex.clone(),
            used: r.used,
        }
    }
}

/// A parsed VS Code theme shown to the user before it is saved.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// Single-use token for [`save_imported_theme`].
    pub token: String,
    /// Theme id the save will create or replace, `imported:<slug>`.
    pub id: String,
    /// Display name, ending in "(imported)".
    pub name: String,
    /// How each role was filled from the file.
    pub rows: Vec<MappingRowView>,
    /// Derived role colours.
    pub roles: RolesView,
    /// Contrast ratios of those roles.
    pub ratios: RatiosView,
    /// Colours that were lifted to reach the contrast target.
    pub adjusted: Vec<String>,
    /// Things the user should know, like an ignored `include`.
    pub notes: Vec<String>,
}

impl ImportPreview {
    fn new(token: String, imp: &VsImport) -> Self {
        Self {
            token,
            id: imp.seed.id.clone(),
            name: imp.seed.name.clone(),
            rows: imp.rows.iter().map(MappingRowView::from).collect(),
            roles: (&imp.resolved.roles).into(),
            ratios: imp.resolved.ratios.into(),
            adjusted: imp.resolved.adjusted.clone(),
            notes: imp.notes.clone(),
        }
    }
}

/// A parsed import waiting for the user to confirm it.
pub struct PendingImport {
    token: String,
    import: VsImport,
}

/// 128-bit token from two `RandomState` hashers (std seeds their keys from the OS).
fn new_token() -> String {
    (0..2u64)
        .map(|salt| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(salt);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Display stem of a picked file's name: directory parts dropped (either separator, or a drive
/// prefix), extension dropped, outer dots trimmed. Only a fallback name; `vscode::parse` cleans it.
fn file_stem(file_name: &str) -> &str {
    let base = file_name.rsplit(['/', '\\', ':']).next().unwrap_or("");
    let stem = match base.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => base,
    };
    stem.trim_matches('.').trim()
}

fn import_error(e: &ThemeError) -> CommandError {
    match e {
        ThemeError::TooLarge => too_large(),
        e => CommandError::new("invalid_theme", e.to_string()),
    }
}

fn too_large() -> CommandError {
    CommandError::new("too_large", "theme file is larger than 1 MB")
}

/// Size check first, then the parser (which checks the size again).
fn prepare(source: &str, file_name: &str) -> Result<VsImport, CommandError> {
    if source.len() > MAX_BYTES {
        return Err(too_large());
    }
    vscode::parse(source, file_stem(file_name)).map_err(|e| import_error(&e))
}

/// Takes the pending import when `token` matches it; any other token leaves the slot alone.
fn take_pending(
    slot: &Mutex<Option<PendingImport>>,
    token: &str,
) -> Result<VsImport, CommandError> {
    let mut slot = slot.lock();
    match slot.take() {
        Some(p) if p.token == token => Ok(p.import),
        other => {
            *slot = other;
            Err(CommandError::new(
                "stale_import",
                "this import is no longer pending; pick the file again",
            ))
        }
    }
}

/// The slug of an `imported:<slug>` id, only when the slug is exactly what `vscode::slug` makes
/// (lowercase `a-z`, `0-9`, single `-`, at most 40 bytes, no bare device name).
fn imported_slug(id: &str) -> Option<&str> {
    id.strip_prefix("imported:")
        .filter(|s| !s.is_empty() && vscode::slug(s) == *s)
}

fn io_error(what: &str, e: &std::io::Error) -> CommandError {
    CommandError::new("io", format!("could not {what} the theme: {e}"))
}

/// Writes `<dir>/<slug>.json` through a temporary file in the same folder; replaces any earlier
/// import with the same slug.
fn write_seed(dir: &Path, seed: &Seed) -> Result<(), CommandError> {
    let slug = imported_slug(&seed.id)
        .ok_or_else(|| CommandError::new("invalid_id", "not an imported theme id"))?;
    let text = serde_json::to_string_pretty(seed)
        .map_err(|e| CommandError::new("internal", e.to_string()))?;
    if text.len() as u64 > MAX_SEED_BYTES {
        return Err(CommandError::new("too_large", "theme is too large to save"));
    }
    let save = |e: std::io::Error| io_error("save", &e);
    fs::create_dir_all(dir).map_err(save)?;
    let tmp = dir.join(format!("{slug}.json.tmp"));
    fs::write(&tmp, text).map_err(save)?;
    fs::rename(&tmp, dir.join(format!("{slug}.json"))).map_err(save)
}

/// Reads at most `MAX_SEED_BYTES`; `None` when the file is larger.
fn read_capped(path: &Path) -> std::io::Result<Option<String>> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(MAX_SEED_BYTES + 1)
        .read_to_string(&mut text)?;
    Ok((text.len() as u64 <= MAX_SEED_BYTES).then_some(text))
}

/// The seed in a saved theme file named `<stem>.json`, or why it is rejected.
fn check_saved(stem: &str, text: &str) -> Result<Seed, String> {
    let seed: Seed = serde_json::from_str(text).map_err(|e| format!("not a theme seed: {e}"))?;
    if imported_slug(&seed.id) != Some(stem) {
        return Err("id does not match the file name".to_owned());
    }
    if seed.source != Source::Imported || seed.upstream.is_some() {
        return Err("not an imported theme".to_owned());
    }
    if seed.name.chars().count() > MAX_SAVED_NAME || seed.name.chars().any(char::is_control) {
        return Err("bad name".to_owned());
    }
    resolve(&seed).map_err(|e| e.to_string())?;
    Ok(seed)
}

/// Every usable seed in the imported-themes folder: at most `MAX_IMPORTED` `.json` files in name
/// order, each at most `MAX_SEED_BYTES`, re-validated with the strict hex parser. A bad file is
/// logged and skipped; a missing folder is empty.
fn load_imported_dir(dir: &Path) -> Vec<Seed> {
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(%e, "imported themes folder unreadable");
            return Vec::new();
        }
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json") && p.is_file())
        .collect();
    paths.sort();
    if paths.len() > MAX_IMPORTED {
        tracing::warn!(
            count = paths.len(),
            "more than {MAX_IMPORTED} imported themes; the rest are ignored"
        );
        paths.truncate(MAX_IMPORTED);
    }
    let mut seeds = Vec::new();
    for path in paths {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let file = file.unwrap_or_default();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let checked = match read_capped(&path) {
            Ok(Some(text)) => check_saved(stem, &text),
            Ok(None) => Err("file is larger than 8 KB".to_owned()),
            Err(e) => Err(format!("unreadable: {e}")),
        };
        match checked {
            Ok(seed) => seeds.push(seed),
            Err(why) => tracing::warn!(file = %file, "imported theme skipped: {why}"),
        }
    }
    seeds
}

fn themes_dir() -> Result<std::path::PathBuf, CommandError> {
    launcher_core::config::themes_dir()
        .map_err(|e| CommandError::new("no_config_dir", e.to_string()))
}

/// Rebuilds the registry from the bundled themes plus the folder, publishes it, returns its views.
fn reload(state: &AppState, dir: &Path) -> Vec<ThemeView> {
    let reg = crate::state::build_themes().with_imported(load_imported_dir(dir));
    let views = theme_views(&reg);
    state.themes.store(Arc::new(reg));
    views
}

/// Tells open windows the theme set changed (an active imported theme may have new colours or be
/// gone, in which case the lookup falls back to the code default).
fn appearance_changed(app: &AppHandle) {
    if let Err(err) = app.emit(
        "settings://changed",
        serde_json::json!({ "section": "appearance" }),
    ) {
        tracing::warn!(%err, "emit failed");
    }
    crate::tray::sync_settings_theme(app);
}

/// Startup: reads the imported-themes folder on the blocking pool and publishes the registry.
pub async fn load_imported(app: &AppHandle, state: Arc<AppState>) {
    let res = blocking(move || {
        reload(&state, &themes_dir()?);
        Ok(())
    })
    .await;
    match res {
        Ok(()) => appearance_changed(app),
        Err(err) => tracing::warn!(%err, "imported themes not loaded"),
    }
}

/// Parses a VS Code colour theme whose text the settings window read with the File API (it never
/// sees a path), and holds it for [`save_imported_theme`], replacing any earlier pending import.
#[tauri::command]
pub async fn preview_vscode_theme(
    state: State<'_, Arc<AppState>>,
    source: String,
    file_name: String,
) -> Result<ImportPreview, CommandError> {
    if source.len() > MAX_BYTES {
        return Err(too_large());
    }
    let s = Arc::clone(&state);
    blocking(move || {
        let import = prepare(&source, &file_name)?;
        let token = new_token();
        let preview = ImportPreview::new(token.clone(), &import);
        *s.pending_import.lock() = Some(PendingImport { token, import });
        Ok(preview)
    })
    .await
}

/// Saves the pending import named by `token` (single use) to the themes folder and returns the
/// fresh theme list. Saving a theme with the same slug again replaces it.
#[tauri::command]
pub async fn save_imported_theme(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    token: String,
) -> Result<Vec<ThemeView>, CommandError> {
    let s = Arc::clone(&state);
    let views = blocking(move || {
        let import = take_pending(&s.pending_import, &token)?;
        let dir = themes_dir()?;
        write_seed(&dir, &import.seed)?;
        Ok(reload(&s, &dir))
    })
    .await?;
    appearance_changed(&app);
    Ok(views)
}

/// Deletes an imported theme (`imported:<slug>` ids only) and returns the fresh theme list. An
/// active slot that named it falls back to the code default.
#[tauri::command]
pub async fn delete_imported_theme(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<ThemeView>, CommandError> {
    let slug = imported_slug(&id)
        .ok_or_else(|| CommandError::new("invalid_id", "not an imported theme id"))?
        .to_owned();
    let s = Arc::clone(&state);
    let views = blocking(move || {
        let dir = themes_dir()?;
        match fs::remove_file(dir.join(format!("{slug}.json"))) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("delete", &e)),
        }
        Ok(reload(&s, &dir))
    })
    .await?;
    appearance_changed(&app);
    Ok(views)
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

/// The theme currently in force for the configured scheme.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTheme {
    /// Theme id.
    pub id: String,
    /// True when the panel is dark.
    pub dark: bool,
    /// Derived role colours.
    pub roles: RolesView,
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
        roles: (&theme.resolved.roles).into(),
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

    const SRC: &str = r##"{"name": "Good", "colors": {
        "editor.background": "#1a1b26", "editor.foreground": "#c0caf5"}}"##;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("flick-themes-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_with(slug: &str) -> Seed {
        let mut seed = prepare(SRC, "f.json").unwrap().seed;
        seed.id = format!("imported:{slug}");
        seed
    }

    fn slot_with(token: &str) -> Mutex<Option<PendingImport>> {
        Mutex::new(Some(PendingImport {
            token: token.to_owned(),
            import: prepare(SRC, "f.json").unwrap(),
        }))
    }

    #[test]
    fn token_is_single_use() {
        let slot = slot_with("abc");
        assert_eq!(take_pending(&slot, "abc").unwrap().seed.id, "imported:good");
        assert_eq!(take_pending(&slot, "abc").unwrap_err().code, "stale_import");
    }

    #[test]
    fn stale_token_is_rejected_and_keeps_the_slot() {
        let slot = slot_with("abc");
        assert_eq!(take_pending(&slot, "old").unwrap_err().code, "stale_import");
        assert_eq!(take_pending(&slot, "").unwrap_err().code, "stale_import");
        assert!(take_pending(&slot, "abc").is_ok());
        let empty = Mutex::new(None);
        assert_eq!(
            take_pending(&empty, "abc").unwrap_err().code,
            "stale_import"
        );
    }

    #[test]
    fn tokens_are_long_and_differ() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn oversize_source_rejected_before_parsing() {
        // Invalid JSONC (unterminated comment) that would fail differently if it were parsed.
        let over = format!("/*{}", " ".repeat(MAX_BYTES));
        assert!(over.len() > MAX_BYTES);
        assert_eq!(prepare(&over, "x.json").err().unwrap().code, "too_large");
        assert_eq!(
            prepare("/* open", "x.json").err().unwrap().code,
            "invalid_theme"
        );
    }

    #[test]
    fn file_name_paths_yield_a_safe_slug() {
        assert_eq!(file_stem(r"..\..\x.json"), "x");
        assert_eq!(file_stem(r"C:\a\b\x.json"), "x");
        assert_eq!(file_stem("../x.jsonc"), "x");
        assert_eq!(file_stem("C:x.json"), "x");
        assert_eq!(file_stem("my.theme.json"), "my.theme");
        assert_eq!(file_stem(".."), "");
        assert_eq!(file_stem(""), "");
        let nameless = r##"{"colors": {
            "editor.background": "#1a1b26", "editor.foreground": "#c0caf5"}}"##;
        for name in [r"..\..\x.json", r"C:\a\b\x.json", "../../x.json"] {
            let imp = prepare(nameless, name).unwrap();
            assert_eq!(imp.seed.id, "imported:x", "{name}");
            assert_eq!(imp.seed.name, "x (imported)", "{name}");
        }
        let dots = prepare(nameless, r"..\..\..").unwrap();
        assert_eq!(dots.seed.id, "imported:imported-theme");
    }

    #[test]
    fn delete_only_accepts_imported_ids() {
        assert_eq!(imported_slug("imported:tokyo-night"), Some("tokyo-night"));
        for bad in [
            "midnight",
            "imported:",
            "imported:../x",
            r"imported:..\x",
            "imported:a/b",
            "imported:A",
            "imported:a--b",
            "imported:-a",
            "imported:con",
            "imported:x.json",
            "port:x",
        ] {
            assert_eq!(imported_slug(bad), None, "{bad}");
        }
        assert_eq!(imported_slug(&format!("imported:{}", "a".repeat(41))), None);
    }

    #[test]
    fn saved_theme_round_trips_and_reimport_overwrites() {
        let dir = tmp("save");
        let mut seed = seed_with("good");
        write_seed(&dir, &seed).unwrap();
        seed.name = "Better (imported)".to_owned();
        write_seed(&dir, &seed).unwrap();
        let loaded = load_imported_dir(&dir);
        assert_eq!(loaded, vec![seed]);
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["good.json"]);
        assert!(write_seed(&dir, &seed_with("../x")).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_saved_seeds_are_skipped() {
        let dir = tmp("invalid");
        let good = seed_with("good");
        write_seed(&dir, &good).unwrap();
        let json = |s: &Seed| serde_json::to_string(s).unwrap();
        let mut colour = seed_with("colour");
        colour.text = "rgb(1,2,3)".to_owned();
        let mut port = seed_with("port");
        port.source = Source::Port;
        let mut link = seed_with("link");
        link.upstream = Some("https://example.com".to_owned());
        let mut ctrl = seed_with("ctrl");
        ctrl.name = "a\u{7}b".to_owned();
        for (file, text) in [
            ("colour.json", json(&colour)),
            ("port.json", json(&port)),
            ("link.json", json(&link)),
            ("ctrl.json", json(&ctrl)),
            ("other.json", json(&seed_with("mismatch"))),
            ("garbage.json", "{not json".to_owned()),
            (
                "big.json",
                format!("{}{}", json(&seed_with("big")), " ".repeat(9000)),
            ),
            (
                "extra.json",
                json(&seed_with("extra")).replace("\"id\"", "\"x\":1,\"id\""),
            ),
            ("good.txt", json(&good)),
        ] {
            fs::write(dir.join(file), text).unwrap();
        }
        assert_eq!(load_imported_dir(&dir), vec![good]);
        assert!(load_imported_dir(&dir.join("missing")).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folder_reads_at_most_100_files() {
        let dir = tmp("cap");
        for i in 0..105 {
            write_seed(&dir, &seed_with(&format!("t{i:03}"))).unwrap();
        }
        let loaded = load_imported_dir(&dir);
        assert_eq!(loaded.len(), MAX_IMPORTED);
        assert_eq!(loaded[0].id, "imported:t000");
        assert_eq!(loaded[99].id, "imported:t099");
        let reg = registry().with_imported(loaded);
        assert!(reg.contains("imported:t050"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn seed_file_cap_is_8_kb() {
        let dir = tmp("size");
        let text = serde_json::to_string(&seed_with("edge")).unwrap();
        let pad = MAX_SEED_BYTES as usize - text.len();
        fs::write(dir.join("edge.json"), format!("{text}{}", " ".repeat(pad))).unwrap();
        assert_eq!(load_imported_dir(&dir).len(), 1);
        fs::write(
            dir.join("edge.json"),
            format!("{text}{}", " ".repeat(pad + 1)),
        )
        .unwrap();
        assert!(load_imported_dir(&dir).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn import_preview_serializes_camel_case() {
        let imp = prepare(SRC, "f.json").unwrap();
        let json = serde_json::to_value(ImportPreview::new("t".to_owned(), &imp)).unwrap();
        for key in [
            "token", "id", "name", "rows", "roles", "ratios", "adjusted", "notes",
        ] {
            assert!(json.get(key).is_some(), "missing {key} in {json}");
        }
        assert_eq!(json["rows"].as_array().unwrap().len(), 7);
        assert!(json["rows"][0].get("used").is_some(), "{json}");
        assert!(json["roles"].get("accentText").is_some(), "{json}");
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
}
