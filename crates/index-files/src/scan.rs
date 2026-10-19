//! Parallel directory scan producing a [`FileStore`].

use crate::error::FilesError;
use crate::skip::should_skip;
use crate::store::{FileStore, NO_PARENT, StoreBuilder};
pub use jwalk::Parallelism;
use jwalk::WalkDirGeneric;
use serde::{Deserialize, Serialize};
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

/// A folder tree to index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Root {
    /// Id stored on every entry under this root.
    pub id: u8,
    /// Absolute folder path.
    pub path: PathBuf,
    /// Display label.
    pub label: String,
}

/// Default roots: `%USERPROFILE%\{Desktop, Documents, Downloads, Pictures, Videos, Music}`
/// that exist. Desktop and Documents redirected by OneDrive live under `%OneDrive%`; that
/// location is used when the profile one is missing.
pub fn default_roots() -> Vec<Root> {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };
    let onedrive = std::env::var_os("OneDrive").map(PathBuf::from);
    let mut roots = Vec::new();
    for label in [
        "Desktop",
        "Documents",
        "Downloads",
        "Pictures",
        "Videos",
        "Music",
    ] {
        let found = std::iter::once(profile.join(label))
            .chain(onedrive.iter().map(|o| o.join(label)))
            .find(|p| p.is_dir());
        if let (Some(path), Ok(id)) = (found, u8::try_from(roots.len())) {
            roots.push(Root {
                id,
                path,
                label: label.to_string(),
            });
        }
    }
    roots
}

fn e_cancelled(c: &AtomicBool) -> bool {
    c.load(Ordering::Relaxed)
}

type Meta = (u32, u32); // (attrs, modified minutes)

fn minutes(md: &std::fs::Metadata) -> u32 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u32::try_from(d.as_secs() / 60).unwrap_or(u32::MAX))
}

/// Scan `roots` into a store using `parallelism`. `cancel` is checked per directory; `progress` gets the running entry count.
pub fn scan(
    roots: &[Root],
    parallelism: &Parallelism,
    cancel: &AtomicBool,
    progress: impl Fn(usize),
) -> Result<FileStore, FilesError> {
    let mut b = StoreBuilder::new();
    for root in roots {
        if cancel.load(Ordering::Relaxed) {
            return Err(FilesError::Cancelled);
        }
        let walk = WalkDirGeneric::<((), Option<Meta>)>::new(&root.path)
            .skip_hidden(false)
            .follow_links(false)
            .parallelism(parallelism.clone())
            .process_read_dir(|_, _, _, children| {
                children.retain_mut(|c| {
                    let Ok(e) = c else { return false };
                    let md = e.metadata().ok();
                    let attrs = md.as_ref().map_or(0, MetadataExt::file_attributes);
                    if should_skip(&e.file_name, attrs, e.file_type.is_dir()) {
                        return false;
                    }
                    e.client_state = md.as_ref().map(|m| (attrs, minutes(m)));
                    true
                });
            });
        let mut stack: Vec<u32> = Vec::new();
        for item in walk {
            if e_cancelled(cancel) {
                return Err(FilesError::Cancelled); // dropping `walk` stops the workers
            }
            let Ok(e) = item else { continue };
            let depth = e.depth;
            stack.truncate(depth);
            let (parent, name) = if depth == 0 {
                (NO_PARENT, root.path.to_string_lossy().into_owned())
            } else {
                match stack.last() {
                    Some(p) => (*p, e.file_name.to_string_lossy().into_owned()),
                    None => continue, // parent was dropped
                }
            };
            let modified = e.client_state.map_or(0, |m| m.1);
            let Some(idx) = b.push(&name, parent, e.file_type.is_dir(), modified, root.id) else {
                continue;
            };
            stack.push(idx);
            if idx % 1024 == 0 {
                progress(idx as usize);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(FilesError::Cancelled);
        }
    }
    let store = b.finish(roots.to_vec());
    progress(store.len());
    Ok(store)
}
