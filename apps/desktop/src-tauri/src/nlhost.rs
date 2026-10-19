//! The shell's side of the natural-language finder: file index access, opening, engine keys.
use crate::state::AppState;
use features::NlHost;
use index_files::{FileHit, FindQuery};
use secrecy::SecretString;
use std::path::{Path, PathBuf};
use std::sync::Weak;

/// Holds the state weakly so the registry (owned by the state) does not keep it alive.
pub struct ShellHost(pub Weak<AppState>);

impl NlHost for ShellHost {
    fn find(&self, q: &FindQuery) -> Vec<FileHit> {
        self.0
            .upgrade()
            .map(|s| s.files.find(&s.overlay.load(), q))
            .unwrap_or_default()
    }

    fn root_id(&self, label: &str) -> Option<u8> {
        let s = self.0.upgrade()?;
        let store = s.files.load();
        store
            .roots()
            .iter()
            .find(|r| r.label.eq_ignore_ascii_case(label))
            .map(|r| r.id)
    }

    fn folder_path(&self, label: &str) -> Option<PathBuf> {
        index_files::scan::default_roots()
            .into_iter()
            .find(|r| r.label.eq_ignore_ascii_case(label))
            .map(|r| r.path)
    }

    fn open(&self, path: &Path) -> Result<(), String> {
        let _com = index_apps::com::ComGuard::new().map_err(|e| e.to_string())?;
        index_apps::launch::open_path(path).map_err(|e| format!("{e:#}"))
    }

    fn engine_key(&self, id: &str, host: &str) -> Option<SecretString> {
        self.0.upgrade()?.vault.engine_key(id, host)
    }
}
