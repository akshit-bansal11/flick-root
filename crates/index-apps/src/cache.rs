//! On-disk app cache (`apps.json`, versioned, atomic writes).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AppEntry, AppsError};

const VERSION: u32 = 1;

#[derive(Serialize)]
struct FileOut<'a> {
    version: u32,
    entries: &'a [AppEntry],
}

#[derive(Deserialize)]
struct FileIn {
    version: u32,
    entries: Vec<AppEntry>,
}

/// `%LOCALAPPDATA%\<APP_DISPLAY_NAME>\cache`, or `None` if `LOCALAPPDATA` is unset.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join(launcher_core::brand::APP_DISPLAY_NAME)
            .join("cache"),
    )
}

/// Write `apps.json` atomically (temp file then rename), creating `dir` if needed.
pub fn save_entries(dir: &Path, entries: &[AppEntry]) -> Result<(), AppsError> {
    fs::create_dir_all(dir).map_err(AppsError::Cache)?;
    let json = serde_json::to_vec(&FileOut {
        version: VERSION,
        entries,
    })
    .map_err(|e| AppsError::Cache(e.into()))?;
    let tmp = dir.join("apps.json.tmp");
    fs::write(&tmp, json).map_err(AppsError::Cache)?;
    fs::rename(&tmp, dir.join("apps.json")).map_err(AppsError::Cache)
}

/// Read `apps.json`. Missing, corrupt or wrong-version files give `Ok(None)`.
pub fn load_entries(dir: &Path) -> Result<Option<Vec<AppEntry>>, AppsError> {
    let bytes = match fs::read(dir.join("apps.json")) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppsError::Cache(e)),
    };
    Ok(serde_json::from_slice::<FileIn>(&bytes)
        .ok()
        .filter(|f| f.version == VERSION)
        .map(|f| f.entries))
}
