use launcher_core::store::{ClearScope, NlCacheEntry, Store};

fn open(dir: &tempfile::TempDir) -> Store {
    let (s, r) = Store::open(&dir.path().join("store.db")).unwrap();
    assert!(!r.reset);
    s
}

#[test]
fn launches_round_trip() {
    let d = tempfile::tempdir().unwrap();
    let s = open(&d);
    s.record_launch("app:a", "app", Some("Ab"), 1_000);
    s.record_launch("app:a", "app", None, 1_000);
    s.flush();
    let rows = s.load_frecency().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].uses, 2);
    assert!((rows[0].score - 2.0).abs() < 1e-9);
    assert_eq!(s.prefix_choice("AB").unwrap().as_deref(), Some("app:a"));
    assert_eq!(s.error_count(), 0);
}

#[test]
fn prefix_choice_overwrite_and_clear() {
    let s = Store::open_in_memory().unwrap();
    s.record_prefix_choice("c", "app:x", 1);
    s.record_prefix_choice("c", "app:y", 2);
    s.record_prefix_choice("c", "app:y", 3);
    s.nl_cache_put(NlCacheEntry {
        cache_key: "k".into(),
        phrase: "p".into(),
        chosen_path: "C:/a".into(),
        engine: "user".into(),
        p: None,
    });
    s.record_launch("app:x", "app", None, 1);
    s.flush();
    assert_eq!(s.prefix_choice("c").unwrap().as_deref(), Some("app:y"));
    assert!(s.nl_cache_get("k").unwrap().is_some());

    s.clear(ClearScope::NlCache);
    s.flush();
    assert!(s.nl_cache_get("k").unwrap().is_none());
    assert_eq!(s.load_frecency().unwrap().len(), 1);

    s.clear(ClearScope::Frecency);
    s.flush();
    assert!(s.load_frecency().unwrap().is_empty());
    assert!(s.prefix_choice("c").unwrap().is_none());
    s.clear(ClearScope::All);
    s.flush();
    assert_eq!(s.error_count(), 0);
}

#[test]
fn corrupt_file_is_reset() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("store.db");
    std::fs::write(&p, vec![0xABu8; 8192]).unwrap();
    let (s, r) = Store::open(&p).unwrap();
    assert!(r.reset);
    let n = std::fs::read_dir(d.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".corrupt-")
        })
        .count();
    assert_eq!(n, 1);
    s.record_launch("app:a", "app", None, 1);
    s.flush();
    assert_eq!(s.load_frecency().unwrap().len(), 1);
}

#[test]
fn concurrent_reads_while_writing() {
    let d = tempfile::tempdir().unwrap();
    let s = open(&d);
    let mut hs = Vec::new();
    for t in 0..4 {
        let s = s.clone();
        hs.push(std::thread::spawn(move || {
            for i in 0..50 {
                s.record_launch(&format!("app:{t}-{i}"), "app", None, i);
                s.load_frecency().unwrap();
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    s.flush();
    assert_eq!(s.load_frecency().unwrap().len(), 200);
    assert_eq!(s.error_count(), 0);
    s.shutdown();
}
