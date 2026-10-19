//! App icons as data URLs, read from the on-disk 32 px cache and memoised.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use index_apps::AppEntry;
use index_apps::icons::IconCache;
use parking_lot::Mutex;
use std::collections::HashMap;

/// Pixel size the launcher asks for and `refresh_apps` warms.
pub const ICON_PX: u32 = 32;

/// Disk icon cache plus an in-memory memo of encoded data URLs.
pub struct Icons {
    cache: IconCache,
    memo: Mutex<HashMap<(String, u32), String>>,
}

impl Icons {
    /// Wraps the disk cache.
    pub fn new(cache: IconCache) -> Self {
        Self {
            cache,
            memo: Mutex::new(HashMap::new()),
        }
    }

    /// The disk cache, for background extraction.
    pub fn cache(&self) -> &IconCache {
        &self.cache
    }

    /// `data:image/png;base64,...` for a warmed icon; memory only, never touches disk.
    pub fn data_url(&self, entry: &AppEntry, size: u32) -> Option<String> {
        self.memo.lock().get(&(entry.id.clone(), size)).cloned()
    }

    /// Extracts (if not on disk) and memoises icons for `entries`. Blocking; for the few shown rows only.
    pub fn extract_and_memo(&self, entries: &[AppEntry], size: u32) {
        let Ok(_com) = index_apps::com::ComGuard::new() else {
            return;
        };
        for e in entries {
            if let Err(err) = self.cache.ensure(e) {
                tracing::debug!(%err, app = %e.name, "icon extraction failed");
                continue;
            }
            let Some(path) = self.cache.icon_path(e, size) else {
                continue;
            };
            if let Ok(bytes) = std::fs::read(path) {
                let url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
                self.memo.lock().insert((e.id.clone(), size), url);
            }
        }
    }

    /// Reads the cached icons of `entries` from disk and replaces the memo.
    /// Blocking: call from a blocking thread.
    pub fn warm(&self, entries: &[AppEntry], size: u32) {
        let memo: HashMap<_, _> = entries
            .iter()
            .filter_map(|e| {
                let bytes = std::fs::read(self.cache.icon_path(e, size)?).ok()?;
                let url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
                Some(((e.id.clone(), size), url))
            })
            .collect();
        *self.memo.lock() = memo;
    }
}

/// `icon_data_url(entry, size)` over `icons`.
pub fn icon_data_url(icons: &Icons, entry: &AppEntry, size: u32) -> Option<String> {
    icons.data_url(entry, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_icon_is_none() {
        let dir = std::env::temp_dir().join("flick-icons-test-none");
        let icons = Icons::new(IconCache::new(&dir));
        let e = AppEntry {
            id: "zz".into(),
            name: "x".into(),
            parsing_name: "x".into(),
            target: None,
            is_packaged: false,
        };
        assert!(icon_data_url(&icons, &e, 32).is_none());
    }
}
