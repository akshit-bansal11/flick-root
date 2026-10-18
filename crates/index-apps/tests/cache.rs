use index_apps::AppEntry;
use index_apps::cache::{load_entries, save_entries};

fn entry() -> AppEntry {
    AppEntry {
        id: "abc".into(),
        name: "Foo".into(),
        parsing_name: "Foo!App".into(),
        target: None,
        is_packaged: true,
    }
}

#[test]
fn round_trip() {
    let d = tempfile::tempdir().unwrap();
    save_entries(d.path(), &[entry()]).unwrap();
    assert_eq!(load_entries(d.path()).unwrap(), Some(vec![entry()]));
}

#[test]
fn missing_is_none() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(load_entries(d.path()).unwrap(), None);
}

#[test]
fn corrupt_and_bad_version_are_none() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("apps.json"), b"{nope").unwrap();
    assert_eq!(load_entries(d.path()).unwrap(), None);
    std::fs::write(
        d.path().join("apps.json"),
        br#"{"version":99,"entries":[]}"#,
    )
    .unwrap();
    assert_eq!(load_entries(d.path()).unwrap(), None);
}
