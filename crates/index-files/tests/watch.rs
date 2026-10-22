//! Watcher integration through notify with a polling deadline.

use index_files::scan::{Parallelism, Root, scan};
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
    ix.replace(
        scan(
            &roots,
            &Parallelism::RayonNewPool(2),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap(),
    );
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

fn root(id: u8, path: &Path) -> Root {
    Root {
        id,
        path: path.to_path_buf(),
        label: "t".into(),
    }
}

fn scan_of(roots: &[Root]) -> index_files::FileStore {
    scan(
        roots,
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
}

fn all_paths(s: &index_files::FileStore) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = (0..s.len()).map(|i| s.path_of(i)).collect();
    v.sort();
    v
}

fn hits(s: &index_files::FileStore, text: &str, root_id: Option<u8>) -> Vec<std::path::PathBuf> {
    let fq = FindQuery {
        root: root_id,
        ..q(text)
    };
    let mut v: Vec<_> = index_files::query(s, &index_files::Overlay::new(), &fq)
        .into_iter()
        .map(|h| h.path)
        .collect();
    v.sort();
    v
}

fn two_trees(p: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (a, b) = (p.join("a"), p.join("b"));
    fs::create_dir_all(a.join("sub")).unwrap();
    fs::create_dir_all(b.join("sub")).unwrap();
    fs::write(a.join("alpha_file.txt"), "x").unwrap();
    fs::write(a.join("sub/beta_file.txt"), "x").unwrap();
    fs::write(b.join("gamma_file.txt"), "x").unwrap();
    fs::write(b.join("sub/delta_file.txt"), "x").unwrap();
    (a, b)
}

#[test]
fn with_root_and_without_root_match_fresh_scans() {
    let d = tempfile::tempdir().unwrap();
    let (a, b) = two_trees(d.path());
    let only_a = scan_of(&[root(0, &a)]);
    let combined = only_a
        .with_root(&root(0, &b), &scan_of(&[root(0, &b)]))
        .unwrap();
    let fresh = scan_of(&[root(0, &a), root(1, &b)]);

    assert_eq!(combined.roots(), fresh.roots(), "ids reassigned 0, 1");
    assert_eq!(all_paths(&combined), all_paths(&fresh));
    for i in 0..combined.len() {
        assert!(
            fs::metadata(combined.path_of(i)).is_ok(),
            "path_of round-trips"
        );
    }
    for text in ["alpha", "beta", "gamma", "delta", "file", "sub"] {
        assert_eq!(
            hits(&combined, text, None),
            hits(&fresh, text, None),
            "{text}"
        );
    }
    assert_eq!(
        hits(&combined, "file", Some(1)),
        hits(&fresh, "file", Some(1))
    );
    assert!(
        hits(&combined, "file", Some(1))
            .iter()
            .all(|p| p.starts_with(&b))
    );

    // Re-adding an existing root replaces it instead of duplicating it.
    let again = combined
        .with_root(&root(0, &b), &scan_of(&[root(0, &b)]))
        .unwrap();
    assert_eq!(all_paths(&again), all_paths(&fresh));

    let dropped = combined.without_root(&b);
    assert_eq!(dropped.roots(), only_a.roots());
    assert_eq!(all_paths(&dropped), all_paths(&only_a));
    assert!(hits(&dropped, "gamma", None).is_empty());

    // Dropping the first root renumbers the second to id 0.
    let first_gone = fresh.without_root(&a);
    assert_eq!(first_gone.roots(), &[root(0, &b)]);
    assert_eq!(hits(&first_gone, "delta", Some(0)).len(), 1);
}

#[test]
fn without_prefix_drops_a_subtree_or_whole_roots() {
    let d = tempfile::tempdir().unwrap();
    let (a, b) = two_trees(d.path());
    let fresh = scan_of(&[root(0, &a), root(1, &b)]);

    let pruned = fresh.without_prefix(&a.join("sub"));
    let paths = all_paths(&pruned);
    assert!(!paths.iter().any(|p| p.starts_with(a.join("sub"))));
    assert!(paths.contains(&a.join("alpha_file.txt")));
    assert!(paths.contains(&b.join("sub/delta_file.txt")));
    assert_eq!(pruned.roots().len(), 2);

    let none = fresh.without_prefix(d.path());
    assert!(none.is_empty());
    assert!(none.roots().is_empty());
}

#[test]
fn missing_root_is_skipped_and_the_rest_are_watched() {
    let d = tempfile::tempdir().unwrap();
    let live = d.path().join("live");
    fs::create_dir_all(&live).unwrap();
    let roots = vec![root(0, &d.path().join("missing")), root(1, &live)];
    let ix = FileIndex::new();
    ix.replace(scan_of(&roots[1..]));
    let w = FileIndex::watch(&roots).unwrap();
    assert_eq!(w.watched(), &roots[1..]);
    assert_eq!(w.failed().len(), 1);
    assert!(!w.is_stale());
    let ov = w.overlay();
    fs::write(live.join("walrus_new.txt"), "x").unwrap();
    assert!(wait(&ix, &ov, "walrus_new", |h| !h.is_empty()));

    assert!(FileIndex::watch(&roots[..1]).is_err(), "no root watchable");
}
