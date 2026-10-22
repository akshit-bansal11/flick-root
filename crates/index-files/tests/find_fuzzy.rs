//! Find is nucleo-fuzzy already: a dropped letter still finds the file (no separate typo fallback).

use index_files::scan::{Parallelism, Root, scan};
use index_files::{FindQuery, Overlay, query};
use std::fs;
use std::sync::atomic::AtomicBool;

#[test]
fn invce_finds_invoice() {
    let d = tempfile::tempdir().unwrap();
    for f in ["invoice.pdf", "photo.png", "notes.txt"] {
        fs::write(d.path().join(f), "x").unwrap();
    }
    let roots = vec![Root {
        id: 0,
        path: d.path().to_path_buf(),
        label: "t".into(),
    }];
    let s = scan(
        &roots,
        &index_files::SkipList::default(),
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let q = FindQuery {
        text: "invce".into(),
        limit: 10,
        ..Default::default()
    };
    let hits = query(&s, &Overlay::new(), &q);
    assert!(hits.iter().any(|h| h.name == "invoice.pdf"));
}
