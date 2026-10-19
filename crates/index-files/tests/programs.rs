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
