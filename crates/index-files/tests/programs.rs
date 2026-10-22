use index_files::programs::{ProgramFile, load_programs, save_programs};
use index_files::{FilesError, Parallelism, scan_programs};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn touch(p: &Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"x").unwrap();
}

#[test]
fn matcher() {
    let p = ProgramFile::from_path(Path::new(r"C:\x\Setup.MSI")).unwrap();
    assert_eq!((p.name.as_str(), p.ext.as_str()), ("Setup", "msi"));
    assert!(ProgramFile::from_path(Path::new("notes.txt")).is_none());
    assert!(ProgramFile::from_path(Path::new("noext")).is_none());
    assert!(ProgramFile::from_path(Path::new("a.appref-ms")).is_some());
}

#[test]
fn scan_filters_and_caches() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    for f in [
        "a.exe",
        "setup.MSI",
        "app.apk",
        "notes.txt",
        "node_modules/x.exe",
        "temp/c.exe",
        "Temp2/d.exe",
        "Start Menu/Programs/s.lnk",
        "hid/b.exe",
        ".claude/shell-snapshots/snap.sh",
    ] {
        touch(&r.join(f));
    }
    // Hidden directories must still be walked (AppData is hidden).
    let _ = std::process::Command::new("attrib")
        .arg("+h")
        .arg(r.join("hid"))
        .status();

    let cancel = AtomicBool::new(false);
    let got = scan_programs(&[r.to_path_buf()], &Parallelism::Serial, &cancel).unwrap();
    let names: Vec<_> = got.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "app", "b", "setup", "d"]);

    let cache = r.join("programs.cache");
    save_programs(&cache, &got).unwrap();
    assert_eq!(load_programs(&cache).unwrap(), Some(got));
    assert_eq!(load_programs(&r.join("missing")).unwrap(), None);
    fs::write(&cache, "flick-programs 1\nC:\\a.exe").unwrap();
    assert_eq!(load_programs(&cache).unwrap(), None);
}

#[test]
fn cancel_returns_cancelled() {
    let d = tempfile::tempdir().unwrap();
    touch(&d.path().join("a.exe"));
    let cancel = AtomicBool::new(true);
    let r = scan_programs(&[d.path().to_path_buf()], &Parallelism::Serial, &cancel);
    assert!(matches!(r, Err(FilesError::Cancelled)));
}

#[test]
fn extra_types_and_app_bundle() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    for f in [
        "a.appinstaller",
        "b.AppImage",
        "c.deb",
        "d.sh",
        "e.pkg",
        "f.dmg",
        "Foo.app/Contents/MacOS/foo",
        "Foo.app/Contents/inner.exe",
    ] {
        touch(&r.join(f));
    }
    let cancel = AtomicBool::new(false);
    let got = scan_programs(&[r.to_path_buf()], &Parallelism::Serial, &cancel).unwrap();
    let names: Vec<_> = got.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "d", "e", "f", "Foo"]);
    let foo = got.iter().find(|p| p.name == "Foo").unwrap();
    assert_eq!(foo.ext, "app");
    assert_eq!(got.iter().find(|p| p.name == "b").unwrap().ext, "appimage");
    let cache = r.join("programs.cache");
    save_programs(&cache, &got).unwrap();
    assert_eq!(load_programs(&cache).unwrap(), Some(got));
}

#[test]
fn hidden_system_root_is_walked() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    touch(&r.join("a.exe"));
    touch(&r.join("sub/b.txt"));
    let attrib = |flags: [&str; 2]| {
        std::process::Command::new("attrib")
            .args(flags)
            .arg(r)
            .status()
            .unwrap()
            .success()
    };
    assert!(attrib(["+h", "+s"]));
    let found = scan_programs(
        &[r.to_path_buf()],
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
    );
    assert!(attrib(["-h", "-s"]));
    let found = found.unwrap();
    assert!(found.iter().any(|p| p.name == "a" && p.ext == "exe"));
}

use index_files::programs::{MAX_PROGRAMS, root_cap, scan_root};
use index_files::{ProgramCache, ScannedRoot};
use std::path::PathBuf;

fn root(path: &str, serial: u32) -> ScannedRoot {
    ScannedRoot {
        root: PathBuf::from(path),
        serial,
        scanned: 1_700_000_000,
        capped: false,
    }
}

fn progs(paths: &[&str]) -> Vec<ProgramFile> {
    paths
        .iter()
        .map(|p| ProgramFile::from_path(Path::new(p)).unwrap())
        .collect()
}

fn names(list: &[ProgramFile]) -> Vec<String> {
    list.iter()
        .map(|p| p.path.to_string_lossy().into_owned())
        .collect()
}

fn two_drives() -> ProgramCache {
    let mut c = ProgramCache::default();
    c.replace_root(root(r"C:\", 0xAB), progs(&[r"C:\b.exe", r"C:\a.exe"]));
    c.replace_root(root(r"D:\", 0x12), progs(&[r"D:\z.exe"]));
    c
}

#[test]
fn v2_round_trip_and_v1_is_none() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("programs.idx");
    let mut c = two_drives();
    c.roots[1].capped = true;
    c.replace_root(
        root(r"E:\with space", 0xDEAD_BEEF),
        progs(&[r"E:\with space\x.exe"]),
    );
    c.save(&path).unwrap();
    assert_eq!(ProgramCache::load(&path).unwrap(), Some(c));

    save_programs(&path, &progs(&[r"C:\a.exe"])).unwrap();
    assert_eq!(ProgramCache::load(&path).unwrap(), None);
    // The v1 loader still reads it, so nothing is wiped before the first v2 scan.
    assert_eq!(load_programs(&path).unwrap().map(|l| l.len()), Some(1));
    assert_eq!(ProgramCache::load(&d.path().join("missing")).unwrap(), None);
}

#[test]
fn corrupt_cache_is_none() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("programs.idx");
    for bad in [
        "garbage",
        "programs v2\nroot C:\\ zz 1 0\nC:\\a.exe",
        "programs v2\nroot C:\\ ab 1 2",
        "programs v2\nroot C:\\ ab notanumber 0",
        "programs v2\nroot  ab 1 0",
        "",
    ] {
        fs::write(&path, bad).unwrap();
        assert_eq!(ProgramCache::load(&path).unwrap(), None, "{bad:?}");
    }
    fs::write(&path, [0xFFu8, 0xFE, 0x00, 0x80]).unwrap();
    assert_eq!(ProgramCache::load(&path).unwrap(), None);
}

#[test]
fn replace_root_keeps_other_roots_and_sort() {
    let mut c = two_drives();
    c.replace_root(
        root(r"C:\", 0xCD),
        progs(&[r"C:\y.exe", r"C:\c.exe", r"D:\stray.exe"]),
    );
    assert_eq!(names(&c.files), [r"C:\c.exe", r"C:\y.exe", r"D:\z.exe"]);
    assert_eq!(c.roots.len(), 2);
    assert_eq!(
        c.roots
            .iter()
            .find(|r| r.letter() == Some('C'))
            .unwrap()
            .serial,
        0xCD
    );
    c.retain(|p| p.name != "y");
    assert_eq!(names(&c.files), [r"C:\c.exe", r"D:\z.exe"]);
}

#[test]
fn visible_hides_detached_and_swapped_volumes() {
    let c = two_drives();
    assert_eq!(c.visible(&[('C', 0xAB), ('D', 0x12)]).len(), 3);
    // D: detached: hidden, but still cached.
    assert_eq!(
        names(&c.visible(&[('C', 0xAB)])),
        [r"C:\a.exe", r"C:\b.exe"]
    );
    assert_eq!(c.files.len(), 3);
    // Another volume on D:.
    assert_eq!(c.visible(&[('C', 0xAB), ('D', 0x99)]).len(), 2);
}

#[test]
fn scan_into_replaces_per_root_and_cancel_keeps_old() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    touch(&a.path().join("one.exe"));
    touch(&b.path().join("two.exe"));
    let roots = vec![(a.path().to_path_buf(), 1), (b.path().to_path_buf(), 2)];
    let mut c = ProgramCache::default();
    let failed = c
        .scan_into(&roots, &Parallelism::Serial, &AtomicBool::new(false))
        .unwrap();
    assert!(failed.is_empty());
    assert_eq!(c.files.len(), 2);

    // Cancel mid-scan: old entries stay.
    touch(&a.path().join("new.exe"));
    let before = c.clone();
    let r = c.scan_into(&roots, &Parallelism::Serial, &AtomicBool::new(true));
    assert!(matches!(r, Err(FilesError::Cancelled)));
    assert_eq!(c, before);

    // A root that vanished keeps its entries; the other root is rescanned.
    let gone = b.path().to_path_buf();
    drop(b);
    let failed = c
        .scan_into(&roots, &Parallelism::Serial, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, gone);
    let got: Vec<_> = c.files.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(got.len(), 3);
    assert!(got.contains(&"new") && got.contains(&"two"));
}

#[test]
fn fair_cap_per_root() {
    assert_eq!(root_cap(0), MAX_PROGRAMS);
    assert_eq!(root_cap(4), MAX_PROGRAMS / 4);
    let d = tempfile::tempdir().unwrap();
    for i in 0..5 {
        touch(&d.path().join(format!("p{i}.exe")));
    }
    let cancel = AtomicBool::new(false);
    let (files, capped) = scan_root(d.path(), 3, &Parallelism::Serial, &cancel).unwrap();
    assert_eq!((files.len(), capped), (3, true));
    let (files, capped) = scan_root(d.path(), 5, &Parallelism::Serial, &cancel).unwrap();
    assert_eq!((files.len(), capped), (5, true));
    let (files, capped) = scan_root(d.path(), 6, &Parallelism::Serial, &cancel).unwrap();
    assert_eq!((files.len(), capped), (5, false));
}
