//! App icons as data URLs, read from the on-disk 32 px cache and memoised.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use index_apps::AppEntry;
use index_apps::icons::IconCache;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};

/// Pixel size the launcher asks for and `refresh_apps` warms.
pub const ICON_PX: u32 = 32;

/// Program-row icons kept in memory at most; the oldest is dropped first.
const PROGRAM_MEMO_MAX: usize = 256;

/// Insertion-ordered, size-bounded map of data URLs for program rows.
#[derive(Default)]
struct BoundedMemo {
    map: HashMap<(String, u32), String>,
    order: VecDeque<(String, u32)>,
}

impl BoundedMemo {
    fn insert(&mut self, key: (String, u32), url: String) {
        if self.map.insert(key.clone(), url).is_none() {
            self.order.push_back(key);
            while self.order.len() > PROGRAM_MEMO_MAX {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }
}

/// Disk icon cache plus in-memory memos of encoded data URLs: the warmed app set, and a
/// bounded one for program rows (they are extracted on demand and must not accumulate).
pub struct Icons {
    cache: IconCache,
    memo: Mutex<HashMap<(String, u32), String>>,
    program_memo: Mutex<BoundedMemo>,
    /// Ids already queued for extraction; a row is queued at most once.
    tried: Mutex<HashSet<String>>,
}

impl Icons {
    /// Wraps the disk cache.
    pub fn new(cache: IconCache) -> Self {
        Self {
            cache,
            memo: Mutex::new(HashMap::new()),
            program_memo: Mutex::new(BoundedMemo::default()),
            tried: Mutex::new(HashSet::new()),
        }
    }

    /// The disk cache, for background extraction.
    pub fn cache(&self) -> &IconCache {
        &self.cache
    }

    /// `data:image/png;base64,...` for a warmed icon; memory only, never touches disk.
    pub fn data_url(&self, entry: &AppEntry, size: u32) -> Option<String> {
        let key = (entry.id.clone(), size);
        if let Some(url) = self.memo.lock().get(&key) {
            return Some(url.clone());
        }
        self.program_memo.lock().map.get(&key).cloned()
    }

    /// The entries that may be extracted now: icon-safe (no remote paths) and never queued before.
    pub fn claim(&self, entries: Vec<AppEntry>) -> Vec<AppEntry> {
        let mut tried = self.tried.lock();
        entries
            .into_iter()
            .filter(|e| index_apps::icons::icon_allowed(e) && tried.insert(e.id.clone()))
            .collect()
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
                self.program_memo.lock().insert((e.id.clone(), size), url);
            }
        }
    }

    /// Reads the cached icons of `entries` from disk and merges them into the app memo.
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
        self.memo.lock().extend(memo);
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

    #[test]
    fn program_memo_drops_the_oldest_past_the_bound() {
        let mut m = BoundedMemo::default();
        for i in 0..PROGRAM_MEMO_MAX + 5 {
            m.insert((i.to_string(), 32), "u".into());
        }
        assert_eq!(m.map.len(), PROGRAM_MEMO_MAX);
        assert!(!m.map.contains_key(&("0".to_owned(), 32)));
        assert!(
            m.map
                .contains_key(&((PROGRAM_MEMO_MAX + 4).to_string(), 32))
        );
        // Re-inserting a live key neither grows nor reorders the memo.
        m.insert(((PROGRAM_MEMO_MAX + 4).to_string(), 32), "v".into());
        assert_eq!(m.order.len(), PROGRAM_MEMO_MAX);
    }

    #[test]
    fn claim_queues_a_row_once_and_skips_remote() {
        let dir = std::env::temp_dir().join("flick-icons-test-claim");
        let icons = Icons::new(IconCache::new(&dir));
        let mk = |id: &str, p: &str| AppEntry {
            id: id.into(),
            name: id.into(),
            parsing_name: p.into(),
            target: None,
            is_packaged: false,
        };
        let local = mk("a", r"C:\x\a.exe");
        assert_eq!(icons.claim(vec![local.clone()]).len(), 1);
        assert!(icons.claim(vec![local]).is_empty());
        assert!(icons.claim(vec![mk("r", r"\\srv\share\r.exe")]).is_empty());
    }
}
