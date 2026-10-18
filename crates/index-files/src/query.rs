//! Fuzzy query over the file store and overlay for Find mode.

use crate::store::FileStore;
use crate::watch::Overlay;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

const CHUNK: usize = 16 * 1024;

/// A Find-mode query. Empty `text` returns the most recently modified matches of the filters.
#[derive(Debug, Clone, Default)]
pub struct FindQuery {
    /// Fuzzy text matched against the file name.
    pub text: String,
    /// Only entries under this root id.
    pub root: Option<u8>,
    /// Allowed extensions (no dot, case-insensitive); empty means any.
    pub exts: Vec<String>,
    /// Only entries modified at or after this Unix time (seconds).
    pub modified_after: Option<i64>,
    /// Only entries modified at or before this Unix time (seconds).
    pub modified_before: Option<i64>,
    /// Only directories.
    pub dirs_only: bool,
    /// Maximum number of hits.
    pub limit: usize,
}

/// One result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHit {
    /// Store index (`store.len() + n` for overlay entries).
    pub idx: usize,
    /// Full path.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// Parent folder path.
    pub parent: String,
    /// Whether it is a directory.
    pub is_dir: bool,
    /// Match score, higher is better.
    pub score: u32,
    /// Modified time, Unix seconds.
    pub modified: i64,
}

impl FindQuery {
    fn has_filters(&self) -> bool {
        self.root.is_some()
            || !self.exts.is_empty()
            || self.modified_after.is_some()
            || self.modified_before.is_some()
            || self.dirs_only
    }

    fn passes(&self, name: &str, is_dir: bool, root: u8, min: u32) -> bool {
        if (self.dirs_only && !is_dir) || self.root.is_some_and(|r| r != root) {
            return false;
        }
        let secs = i64::from(min) * 60;
        if self.modified_after.is_some_and(|t| secs < t)
            || self.modified_before.is_some_and(|t| secs > t)
        {
            return false;
        }
        self.exts.is_empty()
            || name
                .rsplit_once('.')
                .is_some_and(|(_, e)| self.exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
    }
}

struct Scorer {
    pattern: Pattern,
    matcher: Matcher,
    buf: Vec<char>,
    pbuf: Vec<char>,
}

impl Scorer {
    fn new(text: &str) -> Self {
        let mut cfg = Config::DEFAULT.match_paths();
        cfg.prefer_prefix = true;
        Self {
            pattern: Pattern::parse(text, CaseMatching::Smart, Normalization::Smart),
            matcher: Matcher::new(cfg),
            buf: Vec::new(),
            pbuf: Vec::new(),
        }
    }

    /// Name score plus a small bonus when the parent folder name matches too.
    fn score(&mut self, name: &str, parent: &str) -> Option<u32> {
        let s = self
            .pattern
            .score(Utf32Str::new(name, &mut self.buf), &mut self.matcher)?;
        let bonus = self
            .pattern
            .score(Utf32Str::new(parent, &mut self.pbuf), &mut self.matcher)
            .map_or(0, |p| p / 8);
        Some(s + bonus)
    }
}

fn better(a: &(u32, usize), b: &(u32, usize)) -> std::cmp::Ordering {
    b.0.cmp(&a.0).then(a.1.cmp(&b.1))
}

/// Run `q` over `store` merged with `overlay`.
pub fn query(store: &FileStore, overlay: &Overlay, q: &FindQuery) -> Vec<FileHit> {
    let recency = q.text.trim().is_empty();
    if q.limit == 0 || (recency && !q.has_filters()) {
        return Vec::new();
    }
    let ov = overlay.read();
    let removed = &ov.removed;
    let mut cands: Vec<(u32, usize)> = store
        .entries
        .par_chunks(CHUNK)
        .enumerate()
        .map_init(
            || Scorer::new(&q.text),
            |sc, (ci, chunk)| {
                let mut v: Vec<(u32, usize)> = Vec::new();
                for (off, e) in chunk.iter().enumerate() {
                    let idx = ci * CHUNK + off;
                    let name = store.name(idx);
                    if !q.passes(name, e.is_dir, e.root, e.modified_min) {
                        continue;
                    }
                    let score = if recency {
                        Some(e.modified_min)
                    } else {
                        sc.score(name, store.parent(idx).map_or("", |p| store.name(p)))
                    };
                    let Some(score) = score else { continue };
                    if !removed.is_empty()
                        && store.path_of(idx).ancestors().any(|a| removed.contains(a))
                    {
                        continue;
                    }
                    v.push((score, idx));
                }
                if v.len() > q.limit {
                    v.select_nth_unstable_by(q.limit, better);
                    v.truncate(q.limit);
                }
                v
            },
        )
        .flatten()
        .collect();
    let mut sc = Scorer::new(&q.text);
    for (i, a) in ov.added.iter().enumerate() {
        if !q.passes(&a.name, a.is_dir, a.root, a.modified_min) {
            continue;
        }
        let parent = a
            .path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let score = if recency {
            Some(a.modified_min)
        } else {
            sc.score(&a.name, parent)
        };
        if let Some(s) = score {
            cands.push((s, store.len() + i));
        }
    }
    cands.sort_unstable_by(better);
    cands.truncate(q.limit);
    cands
        .into_iter()
        .filter_map(|(score, idx)| {
            if idx < store.len() {
                let path = store.path_of(idx);
                Some(FileHit {
                    idx,
                    name: store.name(idx).to_string(),
                    parent: parent_str(&path),
                    is_dir: store.is_dir(idx),
                    score,
                    modified: i64::from(store.modified_min(idx)?) * 60,
                    path,
                })
            } else {
                let a = ov.added.get(idx - store.len())?;
                Some(FileHit {
                    idx,
                    path: a.path.clone(),
                    name: a.name.clone(),
                    parent: parent_str(&a.path),
                    is_dir: a.is_dir,
                    score,
                    modified: i64::from(a.modified_min) * 60,
                })
            }
        })
        .collect()
}

fn parent_str(p: &Path) -> String {
    p.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
