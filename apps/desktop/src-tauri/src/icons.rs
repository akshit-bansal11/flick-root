//! App icons as data URLs, read from the on-disk 32 px cache and memoised.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use index_apps::AppEntry;
use index_apps::icons::IconCache;
use parking_lot::Mutex;
use std::collections::HashMap;

/// Upper bound on memoised icons before the map is cleared.
const MEMO_CAP: usize = 512;

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

    /// `data:image/png;base64,...` for the cached icon, or `None` if not extracted yet.
    pub fn data_url(&self, entry: &AppEntry, size: u32) -> Option<String> {
        let key = (entry.id.clone(), size);
        if let Some(u) = self.memo.lock().get(&key) {
            return Some(u.clone());
        }
        let bytes = std::fs::read(self.cache.icon_path(entry, size)?).ok()?;
        let url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        let mut m = self.memo.lock();
        if m.len() >= MEMO_CAP {
            m.clear();
        }
        m.insert(key, url.clone());
        Some(url)
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
