//! AppsFolder enumeration, filters, launch (normal, runas, reveal location), icon extraction and the app cache. The only crate besides win allowed unsafe code, each block with a SAFETY comment.

pub mod cache;
pub mod com;
pub mod enumerate;
pub mod error;
pub mod extra;
pub mod filter;
pub mod icons;
pub mod launch;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use error::AppsError;

/// One launchable app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// Stable id: hex of the first 8 bytes of SHA-256 of the parsing name.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Shell parsing name (`shell:AppsFolder\<parsing_name>`).
    pub parsing_name: String,
    /// Executable or link target, if any.
    pub target: Option<PathBuf>,
    /// True for packaged (Store) apps without a target.
    pub is_packaged: bool,
}

impl AppEntry {
    /// What to hand the shell: the parsing name itself when it is an absolute path (extras),
    /// else `shell:AppsFolder\<parsing_name>`.
    pub fn shell_path(&self) -> String {
        if std::path::Path::new(&self.parsing_name).is_absolute() {
            self.parsing_name.clone()
        } else {
            format!("shell:AppsFolder\\{}", self.parsing_name)
        }
    }
}

/// Result of a refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshStats {
    /// Items the shell returned.
    pub raw: usize,
    /// Entries kept after filter and dedupe.
    pub kept: usize,
}

/// Lock-free swappable app list.
pub struct AppIndex {
    entries: ArcSwap<Vec<AppEntry>>,
}

impl Default for AppIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable 16-hex-digit id (SHA-256 prefix) of a parsing name.
pub fn make_id(parsing_name: &str) -> String {
    Sha256::digest(parsing_name.as_bytes())
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl AppIndex {
    /// Empty index.
    pub fn new() -> Self {
        Self {
            entries: ArcSwap::from_pointee(Vec::new()),
        }
    }

    /// Current snapshot.
    pub fn entries(&self) -> Arc<Vec<AppEntry>> {
        self.entries.load_full()
    }

    /// Replace the list (e.g. from the cache).
    pub fn replace(&self, v: Vec<AppEntry>) {
        self.entries.store(Arc::new(v));
    }

    /// Fill the list from `dir`'s cache without enumerating. Returns false if there was no usable cache.
    pub fn load_cache(&self, dir: &std::path::Path) -> Result<bool, AppsError> {
        match cache::load_entries(dir)? {
            Some(v) => {
                self.replace(v);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Write the current list to `dir`'s cache.
    pub fn save_cache(&self, dir: &std::path::Path) -> Result<(), AppsError> {
        cache::save_entries(dir, &self.entries())
    }

    /// Enumerate, filter, dedupe and sort. Blocking and COM-based: call from `spawn_blocking`.
    pub fn refresh_blocking(&self) -> Result<RefreshStats, AppsError> {
        let _com = com::ComGuard::new().map_err(AppsError::Enumerate)?;
        let raw = enumerate::enumerate_raw()?;
        let total = raw.len();
        let mut seen = HashSet::new();
        let mut kept: Vec<enumerate::RawEntry> = raw
            .into_iter()
            .filter(filter::keep)
            .filter(|r| seen.insert((r.name.to_lowercase(), r.parsing_name.clone())))
            .collect();
        // Extras: name filter only (desktop .url must survive); drop repeats by target or name.
        let mut names: HashSet<String> = kept.iter().map(|r| r.name.to_lowercase()).collect();
        let mut targets: HashSet<String> = kept
            .iter()
            .filter_map(|r| r.target.as_ref())
            .map(|t| t.to_string_lossy().to_lowercase())
            .collect();
        for r in extra::extra_entries() {
            let t = r
                .target
                .as_ref()
                .map(|t| t.to_string_lossy().to_lowercase());
            if !filter::keep_name(&r.name)
                || names.contains(&r.name.to_lowercase())
                || t.as_ref().is_some_and(|t| targets.contains(t))
            {
                continue;
            }
            names.insert(r.name.to_lowercase());
            targets.extend(t);
            kept.push(r);
        }
        let mut v: Vec<AppEntry> = kept
            .into_iter()
            .map(|r| AppEntry {
                id: make_id(&r.parsing_name),
                is_packaged: r.target.is_none(),
                name: r.name,
                parsing_name: r.parsing_name,
                target: r.target,
            })
            .collect();
        v.sort_by_key(|a| a.name.to_lowercase());
        let stats = RefreshStats {
            raw: total,
            kept: v.len(),
        };
        self.replace(v);
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(make_id("a"), make_id("a"));
        assert_ne!(make_id("a"), make_id("b"));
    }

    #[test]
    #[ignore = "reads this machine's AppsFolder"]
    fn live_refresh() {
        let idx = AppIndex::new();
        let s = idx.refresh_blocking().unwrap();
        let e = idx.entries();
        let spotify = e.iter().any(|a| a.name.contains("Spotify"));
        let extras = e
            .iter()
            .filter(|a| std::path::Path::new(&a.parsing_name).is_absolute())
            .count();
        println!(
            "raw={} kept={} extras={extras} spotify={spotify}",
            s.raw, s.kept
        );
        assert!(e.len() > 10);
        assert!(e.iter().all(|a| !a.name.contains("Uninstall")));
    }
}
