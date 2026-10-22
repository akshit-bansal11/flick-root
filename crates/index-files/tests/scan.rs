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
        &index_files::SkipList::default(),
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
        &index_files::SkipList::default(),
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
        &index_files::SkipList::default(),
        &Parallelism::RayonNewPool(1),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(s.len(), 3);
}

#[test]
fn hidden_system_root_is_walked() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join("sub")).unwrap();
    fs::write(p.join("sub").join("b.txt"), "x").unwrap();
    let attrib = |flags: [&str; 2]| {
        std::process::Command::new("attrib")
            .args(flags)
            .arg(p)
            .status()
            .unwrap()
            .success()
    };
    assert!(attrib(["+h", "+s"]));
    let roots = [Root {
        id: 0,
        path: p.to_path_buf(),
        label: "t".into(),
    }];
    let s = scan(
        &roots,
        &index_files::SkipList::default(),
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    );
    assert!(attrib(["-h", "-s"]));
    let s = s.unwrap();
    assert!((0..s.len()).any(|i| s.name(i) == "b.txt"));
}

fn root_of(p: &std::path::Path) -> [Root; 1] {
    [Root {
        id: 0,
        path: p.to_path_buf(),
        label: "t".into(),
    }]
}

#[test]
fn excluded_path_and_glob_are_not_indexed() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    for dir in ["keep", "games/deep", "a/scratch_cache/x"] {
        fs::create_dir_all(p.join(dir)).unwrap();
    }
    for f in ["keep/k.txt", "games/deep/g.txt", "a/scratch_cache/x/c.txt"] {
        fs::write(p.join(f), "x").unwrap();
    }
    let skip = index_files::SkipList::new(&[
        p.join("games").to_string_lossy().into_owned(),
        "*_cache".to_string(),
    ]);
    let s = scan(
        &root_of(p),
        &skip,
        &Parallelism::RayonNewPool(2),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let names: Vec<&str> = (0..s.len()).map(|i| s.name(i)).collect();
    assert!(names.contains(&"k.txt"));
    for gone in ["games", "g.txt", "scratch_cache", "c.txt"] {
        assert!(!names.contains(&gone), "{gone} must not be indexed");
    }
}

#[test]
fn cap_stops_the_walk_and_sets_the_flag() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..10 {
        fs::write(d.path().join(format!("f{i}.txt")), "x").unwrap();
    }
    let run = |max| {
        index_files::scan_capped(
            &root_of(d.path()),
            &index_files::SkipList::default(),
            max,
            &Parallelism::Serial,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap()
    };
    let (s, capped) = run(4);
    assert!(capped);
    assert_eq!(s.len(), 4);
    let (s, capped) = run(11);
    assert!(!capped, "root plus ten files fit exactly");
    assert_eq!(s.len(), 11);
}
