use index_files::scan::{Parallelism, Root, scan};
use std::fs;
use std::sync::atomic::AtomicBool;

#[test]
fn skips_and_keeps() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    for dir in ["node_modules/x", ".git/objects", "src/nested/deep"] {
        fs::create_dir_all(p.join(dir)).unwrap();
    }
    for f in [
        "node_modules/x/a.js",
        ".git/HEAD",
        "src/main.rs",
        "src/nested/deep/n.txt",
        "hidden.txt",
    ] {
        fs::write(p.join(f), "x").unwrap();
    }
    let st = std::process::Command::new("attrib")
        .arg("+h")
        .arg(p.join("hidden.txt"))
        .status()
        .unwrap();
    assert!(st.success());

    let roots = [Root {
        id: 0,
        path: p.to_path_buf(),
        label: "t".into(),
    }];
    let s = scan(
        &roots,
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let mut names: Vec<String> = (0..s.len()).map(|i| s.name(i).to_string()).collect();
    names.remove(0); // root itself
    names.sort();
    assert_eq!(names, ["deep", "main.rs", "n.txt", "nested", "src"]);
    let n = (0..s.len()).find(|i| s.name(*i) == "n.txt").unwrap();
    assert_eq!(
        s.path_of(n),
        p.join("src").join("nested").join("deep").join("n.txt")
    );
}

#[test]
fn cancelled() {
    let d = tempfile::tempdir().unwrap();
    let roots = [Root {
        id: 0,
        path: d.path().to_path_buf(),
        label: "t".into(),
    }];
    let r = scan(
        &roots,
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(true),
        |_| {},
    );
    assert!(matches!(r, Err(index_files::FilesError::Cancelled)));
}

#[test]
fn single_thread_pool_still_finds_files() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("sub")).unwrap();
    std::fs::write(d.path().join("sub").join("alpha.txt"), b"x").unwrap();
    let roots = [Root {
        id: 0,
        path: d.path().to_path_buf(),
        label: "t".into(),
    }];
    let s = scan(
        &roots,
        &Parallelism::RayonNewPool(1),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(s.len(), 3);
}
