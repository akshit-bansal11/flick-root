use index_files::programs::ProgramFile;
use index_files::{FilesError, Parallelism, SkipList};
use launcher_core::config::AppsConfig;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// One root walked with the full cap (what the app does per drive).
fn scan(r: &Path, par: &Parallelism, cancel: &AtomicBool) -> Result<Vec<ProgramFile>, FilesError> {
    scan_with(r, par, cancel, &default_skip())
}

fn default_skip() -> SkipList {
    SkipList::new(&AppsConfig::default().skip)
}

fn scan_with(
    r: &Path,
    par: &Parallelism,
    cancel: &AtomicBool,
    skip: &SkipList,
) -> Result<Vec<ProgramFile>, FilesError> {
    scan_root(r, MAX_PROGRAMS, par, cancel, skip).map(|(v, _)| v)
}

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
        "Windows/w.exe",
    ] {
        touch(&r.join(f));
    }
    // Hidden directories must still be walked (AppData is hidden).
    let _ = std::process::Command::new("attrib")
        .arg("+h")
        .arg(r.join("hid"))
        .status();

    let cancel = AtomicBool::new(false);
    let got = scan(r, &Parallelism::Serial, &cancel).unwrap();
    let names: Vec<_> = got.iter().map(|p| p.name.as_str()).collect();
    // Default rules index temp, dot-folders, Start Menu and Windows; only node_modules is
    // skipped. Sorted by lowercase path ("temp2\\" sorts before "temp\\").
    assert_eq!(
        names,
        ["snap", "a", "app", "b", "setup", "s", "d", "c", "w"]
    );
}

#[test]
fn user_skip_list_and_os_internals() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    for f in [
        "keep/a.exe",
        "target/debug/app.exe",
        "target/debug/deps/t-1.exe",
        "target/debug/build/b.exe",
        "target/release/incremental/i.exe",
        "$RECYCLE.BIN/r.exe",
        "System Volume Information/s.exe",
        "Games/g.exe",
        "x/cache1/c.exe",
    ] {
        touch(&r.join(f));
    }
    let cancel = AtomicBool::new(false);
    let par = Parallelism::Serial;
    let names = |skip: &SkipList| -> Vec<String> {
        scan_with(r, &par, &cancel, skip)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect()
    };
    // Defaults: Cargo intermediates skipped, target debug app.exe kept; OS internals always.
    assert_eq!(names(&default_skip()), ["g", "a", "app", "c"]);
    // An empty list excludes nothing of the user's, but OS internals stay hidden.
    let none = SkipList::new(&[]);
    assert_eq!(names(&none).len(), 7);
    assert!(!names(&none).contains(&"r".to_string()));
    assert!(!names(&none).contains(&"s".to_string()));
    // Hand-built list: a name glob and a path prefix under the root.
    let custom = SkipList::new(&[
        "*cache*".to_string(),
        r.join("games").to_string_lossy().into_owned(),
    ]);
    let got = names(&custom);
    assert!(!got.contains(&"c".to_string()) && !got.contains(&"g".to_string()));
    assert!(got.contains(&"a".to_string()));
}

#[test]
fn cancel_returns_cancelled() {
    let d = tempfile::tempdir().unwrap();
    touch(&d.path().join("a.exe"));
    let cancel = AtomicBool::new(true);
    let r = scan(d.path(), &Parallelism::Serial, &cancel);
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
    let got = scan(r, &Parallelism::Serial, &cancel).unwrap();
    let names: Vec<_> = got.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "d", "e", "f", "Foo"]);
    let foo = got.iter().find(|p| p.name == "Foo").unwrap();
    assert_eq!(foo.ext, "app");
    assert_eq!(got.iter().find(|p| p.name == "b").unwrap().ext, "appimage");
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
    let found = scan(r, &Parallelism::RayonNewPool(2), &AtomicBool::new(false));
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

    // A v1 cache (flat list) is not v2: ignored, never an error; the first v2 scan replaces it.
    fs::write(&path, "programs v1\nC:\\a.exe").unwrap();
    assert_eq!(ProgramCache::load(&path).unwrap(), None);
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
        .scan_into(
            &roots,
            2,
            &Parallelism::Serial,
            &AtomicBool::new(false),
            &default_skip(),
        )
        .unwrap();
    assert!(failed.is_empty());
    assert_eq!(c.files.len(), 2);

    // Cancel mid-scan: old entries stay.
    touch(&a.path().join("new.exe"));
    let before = c.clone();
    let r = c.scan_into(
        &roots,
        2,
        &Parallelism::Serial,
        &AtomicBool::new(true),
        &default_skip(),
    );
    assert!(matches!(r, Err(FilesError::Cancelled)));
    assert_eq!(c, before);

    // A root that vanished keeps its entries; the other root is rescanned.
    let gone = b.path().to_path_buf();
    drop(b);
    let failed = c
        .scan_into(
            &roots,
            2,
            &Parallelism::Serial,
            &AtomicBool::new(false),
            &default_skip(),
        )
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, gone);
    let got: Vec<_> = c.files.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(got.len(), 3);
    assert!(got.contains(&"new") && got.contains(&"two"));
}

#[test]
fn single_drive_rescan_uses_the_share_of_all_drives() {
    let d = tempfile::tempdir().unwrap();
    touch(&d.path().join("one.exe"));
    let mut c = ProgramCache::default();
    // The cap comes from the drive count passed in, not from this call's one root.
    let failed = c
        .scan_into(
            &[(d.path().to_path_buf(), 1)],
            MAX_PROGRAMS * 2,
            &Parallelism::Serial,
            &AtomicBool::new(false),
            &default_skip(),
        )
        .unwrap();
    assert!(failed.is_empty());
    assert_eq!(c.roots.len(), 1);
    assert_eq!(root_cap(MAX_PROGRAMS * 2), 0);
    assert_eq!(root_cap(4), MAX_PROGRAMS / 4);
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
    let (files, capped) =
        scan_root(d.path(), 3, &Parallelism::Serial, &cancel, &default_skip()).unwrap();
    assert_eq!((files.len(), capped), (3, true));
    let (files, capped) =
        scan_root(d.path(), 5, &Parallelism::Serial, &cancel, &default_skip()).unwrap();
    assert_eq!((files.len(), capped), (5, true));
    let (files, capped) =
        scan_root(d.path(), 6, &Parallelism::Serial, &cancel, &default_skip()).unwrap();
    assert_eq!((files.len(), capped), (5, false));
}
