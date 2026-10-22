use index_files::scan::{Parallelism, Root, scan};
use index_files::{FileHit, FileIndex, FindQuery, Overlay, query};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn setup(p: &Path) -> Vec<Root> {
    fs::create_dir_all(p.join("docs/reports")).unwrap();
    for f in [
        "docs/reports/budget_2024.xlsx",
        "docs/reports/invoice_2024.pdf",
        "docs/notes.txt",
        "photo.png",
    ] {
        fs::write(p.join(f), "x").unwrap();
    }
    vec![Root {
        id: 0,
        path: p.to_path_buf(),
        label: "t".into(),
    }]
}

fn q(text: &str) -> FindQuery {
    FindQuery {
        text: text.into(),
        limit: 10,
        ..Default::default()
    }
}

fn names(hits: &[FileHit]) -> Vec<&str> {
    hits.iter().map(|h| h.name.as_str()).collect()
}

#[test]
fn text_ext_date_filters() {
    let d = tempfile::tempdir().unwrap();
    let roots = setup(d.path());
    let s = scan(
        &roots,
        &index_files::SkipList::default(),
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let ov = Overlay::new();
    let h = query(&s, &ov, &q("budget xlsx"));
    assert_eq!(names(&h), ["budget_2024.xlsx"]);
    assert!(h[0].path.ends_with("docs\\reports\\budget_2024.xlsx"));
    assert!(query(&s, &ov, &q("")).is_empty());

    let mut f = q("2024");
    f.exts = vec!["PDF".into()];
    assert_eq!(names(&query(&s, &ov, &f)), ["invoice_2024.pdf"]);

    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    let mut f = q("");
    f.modified_after = Some(now - 3600);
    f.exts = vec!["png".into()];
    assert_eq!(names(&query(&s, &ov, &f)), ["photo.png"]);
    f.modified_after = None;
    f.modified_before = Some(now - 86_400 * 365);
    assert!(query(&s, &ov, &f).is_empty());

    let mut f = q("");
    f.dirs_only = true;
    let dirs = query(&s, &ov, &f);
    assert!(!dirs.is_empty() && dirs.iter().all(|h| h.is_dir));
}

#[test]
fn overlay_create_and_delete() {
    let d = tempfile::tempdir().unwrap();
    let roots = setup(d.path());
    let s = scan(
        &roots,
        &index_files::SkipList::default(),
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let ix = FileIndex::new();
    ix.replace(s);
    let w = FileIndex::watch(&roots, &index_files::SkipList::default(), None).unwrap();
    let ov = w.overlay();
    let poll = |want: bool, text: &str| {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if ix.find(&ov, &q(text)).is_empty() != want {
                return true;
            }
            if Instant::now() > end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    fs::write(d.path().join("docs/zebra_new.txt"), "x").unwrap();
    assert!(poll(true, "zebra_new"), "created file appears");
    fs::remove_file(d.path().join("docs/zebra_new.txt")).unwrap();
    assert!(poll(false, "zebra_new"), "deleted overlay file disappears");
    fs::remove_file(d.path().join("photo.png")).unwrap();
    assert!(poll(false, "photo"), "deleted store file disappears");
    fs::write(d.path().join("docs/after.txt"), "x").unwrap();
    assert!(poll(true, "after"));
    ix.compact(&ov);
    assert!(ov.is_empty());
    assert!(!ix.find(&ov, &q("after")).is_empty());
    assert!(ix.find(&ov, &q("photo")).is_empty());
}
