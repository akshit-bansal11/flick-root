//! Versioned little-endian binary snapshot of a [`FileStore`].
//!
//! Layout: magic `FLKI`, version u32, root count u32, roots (id u8, path, label as u32-len bytes),
//! arena (u64 len + bytes), entry count u32, entries (16 bytes each).

use crate::error::FilesError;
use crate::scan::Root;
use crate::store::{Entry, FileStore, NO_PARENT};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"FLKI";
const VERSION: u32 = 1;

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

/// Atomically write `store` to `path` (tmp file + rename).
pub fn save(path: &Path, store: &FileStore) -> Result<(), FilesError> {
    let mut out = Vec::with_capacity(store.names.len() + store.entries.len() * 16 + 64);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(store.roots.len() as u32).to_le_bytes());
    for r in &store.roots {
        out.push(r.id);
        put_bytes(&mut out, r.path.to_string_lossy().as_bytes());
        put_bytes(&mut out, r.label.as_bytes());
    }
    out.extend_from_slice(&(store.names.len() as u64).to_le_bytes());
    out.extend_from_slice(store.names.as_bytes());
    out.extend_from_slice(&(store.entries.len() as u32).to_le_bytes());
    for e in &store.entries {
        out.extend_from_slice(&e.name_off.to_le_bytes());
        out.extend_from_slice(&e.name_len.to_le_bytes());
        out.push(u8::from(e.is_dir));
        out.push(e.root);
        out.extend_from_slice(&e.parent.to_le_bytes());
        out.extend_from_slice(&e.modified_min.to_le_bytes());
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &out)
        .map_err(|e| FilesError::io(format!("write {}", tmp.display()), e))?;
    std::fs::rename(&tmp, path).map_err(|e| FilesError::io(format!("rename {}", path.display()), e))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (a, b) = self.0.split_at_checked(n)?;
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8)
            .and_then(|b| b.try_into().ok())
            .map(u64::from_le_bytes)
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

fn parse(data: &[u8]) -> Option<FileStore> {
    let mut r = Reader(data);
    if r.take(4)? != MAGIC || r.u32()? != VERSION {
        return None;
    }
    let nroots = r.u32()?;
    let mut roots = Vec::new();
    for _ in 0..nroots {
        let id = r.u8()?;
        let path = PathBuf::from(r.string()?);
        roots.push(Root {
            id,
            path,
            label: r.string()?,
        });
    }
    let nlen = usize::try_from(r.u64()?).ok()?;
    let names = String::from_utf8(r.take(nlen)?.to_vec()).ok()?;
    let count = r.u32()?;
    if (count as usize).checked_mul(16)? > r.0.len() {
        return None;
    }
    let mut entries = Vec::with_capacity(count as usize);
    for i in 0..count {
        let name_off = r.u32()?;
        let name_len = r.u16()?;
        let is_dir = r.u8()? != 0;
        let root = r.u8()?;
        let parent = r.u32()?;
        let modified_min = r.u32()?;
        let end = (name_off as usize).checked_add(name_len as usize)?;
        names.get(name_off as usize..end)?; // in bounds and on char boundaries
        if parent != NO_PARENT && parent >= i {
            return None; // parents precede children
        }
        entries.push(Entry {
            name_off,
            name_len,
            is_dir,
            root,
            parent,
            modified_min,
        });
    }
    Some(FileStore {
        roots,
        names,
        entries,
    })
}

/// Load a snapshot. Missing, wrong-version or corrupt files yield `Ok(None)`.
pub fn load(path: &Path) -> Result<Option<FileStore>, FilesError> {
    match std::fs::read(path) {
        Ok(data) => Ok(parse(&data)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(FilesError::io(format!("read {}", path.display()), e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StoreBuilder;

    #[test]
    fn round_trip_and_bad_input() {
        let mut b = StoreBuilder::new();
        let r = b.push("C:\\r", NO_PARENT, true, 5, 0).unwrap();
        b.push("é.txt", r, false, 9, 0).unwrap();
        let s = b.finish(vec![Root {
            id: 0,
            path: "C:\\r".into(),
            label: "r".into(),
        }]);
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.bin");
        save(&p, &s).unwrap();
        assert_eq!(load(&p).unwrap().unwrap(), s);
        assert!(load(&dir.path().join("none")).unwrap().is_none());
        let mut bytes = std::fs::read(&p).unwrap();
        bytes[4] = 99; // version
        std::fs::write(&p, &bytes).unwrap();
        assert!(load(&p).unwrap().is_none());
        std::fs::write(&p, b"FLKI").unwrap();
        assert!(load(&p).unwrap().is_none());
    }
}
