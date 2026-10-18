//! Watcher integration through notify with a polling deadline.

use index_files::scan::{Root, scan};
use index_files::{FileHit, FileIndex, FindQuery};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn q(text: &str) -> FindQuery {
    FindQuery {
        text: text.into(),
        limit: 20,
        ..Default::default()
    }
}

/// Poll until the hits satisfy `ok` or 3 s pass; returns whether they did.
fn wait(
    ix: &FileIndex,
    ov: &index_files::Overlay,
    text: &str,
    ok: impl Fn(&[FileHit]) -> bool,
) -> bool {
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        if ok(&ix.find(ov, &q(text))) {
            return true;
        }
        if Instant::now() > end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn setup(p: &Path) -> Vec<Root> {
    fs::create_dir_all(p.join("old/inner")).unwrap();
    fs::write(p.join("old/inner/kiwi_child.txt"), "x").unwrap();
    fs::create_dir_all(p.join("gone")).unwrap();
    fs::write(p.join("gone/mango_child.txt"), "x").unwrap();
    fs::create_dir_all(p.join("hid")).unwrap();
    vec![Root {
        id: 0,
        path: p.to_path_buf(),
        label: "t".into(),
    }]
}

#[test]
fn watcher_lifecycle() {
    let d = tempfile::tempdir().unwrap();
    let roots = setup(d.path());
    let ix = FileIndex::new();
    ix.replace(scan(&roots, &AtomicBool::new(false), |_| {}).unwrap());
    let w = FileIndex::watch(&roots).unwrap();
    let ov = w.overlay();
    let has = |h: &[FileHit]| !h.is_empty();
    let none = |h: &[FileHit]| h.is_empty();

    fs::write(d.path().join("zebra_new.txt"), "x").unwrap();
    assert!(wait(&ix, &ov, "zebra_new", has), "created file appears");

    fs::rename(d.path().join("old"), d.path().join("renamed")).unwrap();
    assert!(
        wait(&ix, &ov, "kiwi_child", |h| h.len() == 1
            && h[0].path.starts_with(d.path().join("renamed"))),
        "children move with the renamed folder"
    );

    fs::remove_dir_all(d.path().join("gone")).unwrap();
    assert!(
        wait(&ix, &ov, "mango_child", none),
        "deleted folder's children disappear"
    );

    // Hidden folder: attrib +h, then create a file inside it.
    let hid = d.path().join("hid");
    let st = std::process::Command::new("attrib")
        .arg("+h")
        .arg(&hid)
        .status()
        .unwrap();
    assert!(st.success());
    fs::write(hid.join("secret_inside.txt"), "x").unwrap();
    fs::write(d.path().join("sentinel_visible.txt"), "x").unwrap();
    assert!(wait(&ix, &ov, "sentinel_visible", has), "sentinel arrives");
    assert!(
        ix.find(&ov, &q("secret_inside")).is_empty(),
        "hidden folder content stays out"
    );

    drop(w);
    std::thread::sleep(Duration::from_millis(300));
    fs::write(d.path().join("after_drop.txt"), "x").unwrap();
    assert!(
        !wait_short(&ix, &ov, "after_drop"),
        "no events after the handle is dropped"
    );
}

fn wait_short(ix: &FileIndex, ov: &index_files::Overlay, text: &str) -> bool {
    let end = Instant::now() + Duration::from_millis(1000);
    while Instant::now() < end {
        if !ix.find(ov, &q(text)).is_empty() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}
