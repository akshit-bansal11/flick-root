//! Non-secret secret metadata, persisted as JSON next to the store.
//!
//! ponytail: the launcher-core `Store` has a `secret_meta` table but no accessors,
//! so metadata lives in `secret_meta.json` (name, host, timestamps; never values).
//! Move to the table once `Store` exposes it.
use parking_lot::Mutex;
use providers::secret::{MetaStore, SecretMeta, VaultError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct Row {
    name: String,
    host: String,
    added_at: i64,
    last_used: Option<i64>,
}

/// JSON-file backed [`MetaStore`].
pub struct JsonMeta {
    path: PathBuf,
    map: Mutex<BTreeMap<String, SecretMeta>>,
}

impl JsonMeta {
    /// Opens the file at `path`; a missing or unreadable file starts empty.
    pub fn open(path: PathBuf) -> Self {
        let rows: Vec<Row> = match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|err| {
                tracing::warn!(%err, "secret metadata unreadable, starting empty");
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        let map = rows
            .into_iter()
            .map(|r| {
                let m = SecretMeta {
                    name: r.name,
                    host: r.host,
                    added_at: r.added_at,
                    last_used: r.last_used,
                };
                (m.name.clone(), m)
            })
            .collect();
        Self {
            path,
            map: Mutex::new(map),
        }
    }

    fn save(&self, map: &BTreeMap<String, SecretMeta>) -> Result<(), VaultError> {
        let rows: Vec<Row> = map
            .values()
            .map(|m| Row {
                name: m.name.clone(),
                host: m.host.clone(),
                added_at: m.added_at,
                last_used: m.last_used,
            })
            .collect();
        let bytes =
            serde_json::to_vec_pretty(&rows).map_err(|e| VaultError::Meta(e.to_string()))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)
            .and_then(|()| std::fs::rename(&tmp, &self.path))
            .map_err(|e| VaultError::Meta(e.to_string()))
    }
}

impl MetaStore for JsonMeta {
    fn put(&self, meta: SecretMeta) -> Result<(), VaultError> {
        let mut m = self.map.lock();
        m.insert(meta.name.clone(), meta);
        self.save(&m)
    }
    fn get(&self, name: &str) -> Result<Option<SecretMeta>, VaultError> {
        Ok(self.map.lock().get(name).cloned())
    }
    fn remove(&self, name: &str) -> Result<(), VaultError> {
        let mut m = self.map.lock();
        if m.remove(name).is_some() {
            self.save(&m)?;
        }
        Ok(())
    }
    fn touch(&self, name: &str, last_used: i64) -> Result<(), VaultError> {
        let mut m = self.map.lock();
        match m.get_mut(name) {
            Some(e) => e.last_used = Some(last_used),
            None => return Ok(()),
        }
        self.save(&m)
    }
    fn list(&self) -> Result<Vec<SecretMeta>, VaultError> {
        Ok(self.map.lock().values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!("flick-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("secret_meta.json");
        let m = JsonMeta::open(p.clone());
        m.put(SecretMeta {
            name: "provider.x@x.test".into(),
            host: "x.test".into(),
            added_at: 5,
            last_used: None,
        })
        .unwrap();
        assert_eq!(JsonMeta::open(p.clone()).list().unwrap().len(), 1);
        m.remove("provider.x@x.test").unwrap();
        assert!(JsonMeta::open(p).list().unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
