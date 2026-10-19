use super::*;
use decision::{ChoiceAnswer, EngineError, ProbeResult};
use launcher_core::store::Store;

const PHRASE: &str = "the tax invoice pdf from last week";

struct Host {
    hits: Vec<FileHit>,
    opened: Mutex<Vec<PathBuf>>,
    queries: Mutex<Vec<FindQuery>>,
}

impl NlHost for Host {
    fn find(&self, q: &FindQuery) -> Vec<FileHit> {
        self.queries.lock().push(q.clone());
        self.hits.clone()
    }
    fn root_id(&self, label: &str) -> Option<u8> {
        (label == "Downloads").then_some(2)
    }
    fn open(&self, path: &Path) -> Result<(), String> {
        self.opened.lock().push(path.to_path_buf());
        Ok(())
    }
    fn engine_key(&self, _: &str, _: &str) -> Option<SecretString> {
        None
    }
}

fn hit(name: &str, score: u32) -> FileHit {
    FileHit {
        idx: 0,
        path: PathBuf::from(format!("C:\\Users\\a\\Downloads\\{name}")),
        name: name.into(),
        parent: "C:\\Users\\a\\Downloads".into(),
        is_dir: false,
        score,
        modified: 1_700_000_000,
    }
}

fn host(hits: Vec<FileHit>) -> Arc<Host> {
    Arc::new(Host {
        hits,
        opened: Mutex::default(),
        queries: Mutex::default(),
    })
}

struct Fake {
    index: usize,
    p: f32,
    seen: Mutex<Vec<ChoiceQuestion>>,
}

struct Shared(Arc<Fake>);

#[async_trait]
impl DecisionEngine for Shared {
    fn id(&self) -> &'static str {
        "laya"
    }
    async fn choose(&self, q: &ChoiceQuestion) -> Result<ChoiceAnswer, EngineError> {
        self.0.seen.lock().push(q.clone());
        Ok(ChoiceAnswer {
            index: self.0.index,
            p: self.0.p,
            engine: "laya",
        })
    }
    async fn probe(&self) -> ProbeResult {
        ProbeResult {
            id: "laya".into(),
            reachable: true,
            detail: None,
        }
    }
}

fn engine(index: usize, p: f32) -> (Arc<Fake>, Vec<Box<dyn DecisionEngine>>) {
    let f = Arc::new(Fake {
        index,
        p,
        seen: Mutex::default(),
    });
    (f.clone(), vec![Box::new(Shared(f))])
}

async fn run(h: &Arc<Host>, store: &Store, engines: &[Box<dyn DecisionEngine>]) -> Outcome {
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    resolve(h.clone(), store, PHRASE, today, engines, 0.6, 8)
        .await
        .unwrap()
}

fn opened(h: &Host) -> Vec<String> {
    h.opened
        .lock()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn clear_pdf_winner_opens() {
    let h = host(vec![hit("tax-invoice.pdf", 300), hit("notes.pdf", 100)]);
    let s = Store::open_in_memory().unwrap();
    assert_eq!(run(&h, &s, &[]).await, Outcome::HideLauncher);
    assert_eq!(opened(&h), ["tax-invoice.pdf"]);
    let q = h.queries.lock()[0].clone();
    assert!(q.exts.iter().any(|e| e == "pdf"));
    assert!(q.modified_after.is_some() && q.modified_before.is_some());
}

#[tokio::test]
async fn executable_top_hit_goes_to_list() {
    let h = host(vec![hit("invoice.exe", 900), hit("invoice.pdf", 100)]);
    let s = Store::open_in_memory().unwrap();
    let Outcome::ShowItems { items } = run(&h, &s, &[]).await else {
        panic!("expected list");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "invoice.exe");
    assert!(opened(&h).is_empty());
}

#[tokio::test]
async fn close_scores_with_engines_off_list() {
    let h = host(vec![hit("a.pdf", 300), hit("b.pdf", 290)]);
    let s = Store::open_in_memory().unwrap();
    assert!(matches!(run(&h, &s, &[]).await, Outcome::ShowItems { .. }));
    assert!(opened(&h).is_empty());
}

#[tokio::test]
async fn engine_choice_used_when_confident() {
    let h = host(vec![hit("a.pdf", 300), hit("b.pdf", 290)]);
    let s = Store::open_in_memory().unwrap();
    let (fake, engines) = engine(1, 0.9);
    assert_eq!(run(&h, &s, &engines).await, Outcome::HideLauncher);
    assert_eq!(opened(&h), ["b.pdf"]);
    // Only name and parent folder are sent.
    let q = fake.seen.lock()[0].clone();
    assert_eq!(q.phrase, PHRASE);
    assert_eq!(
        q.candidates,
        vec![
            Candidate {
                name: "a.pdf".into(),
                parent: "C:\\Users\\a\\Downloads".into()
            },
            Candidate {
                name: "b.pdf".into(),
                parent: "C:\\Users\\a\\Downloads".into()
            },
        ]
    );
}

#[tokio::test]
async fn engine_choice_ignored_below_threshold_or_unsafe_type() {
    let s = Store::open_in_memory().unwrap();
    let h = host(vec![hit("a.pdf", 300), hit("b.pdf", 290)]);
    let (_, engines) = engine(1, 0.59);
    assert!(matches!(
        run(&h, &s, &engines).await,
        Outcome::ShowItems { .. }
    ));
    let h = host(vec![hit("a.pdf", 300), hit("run.bat", 290)]);
    let (_, engines) = engine(1, 0.99);
    assert!(matches!(
        run(&h, &s, &engines).await,
        Outcome::ShowItems { .. }
    ));
    assert!(opened(&h).is_empty());
}

#[tokio::test]
async fn cache_hit_skips_search() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("cached.pdf");
    std::fs::write(&file, b"x").unwrap();
    let s = Store::open_in_memory().unwrap();
    s.nl_cache_put(NlCacheEntry {
        cache_key: cache_key(PHRASE),
        phrase: PHRASE.into(),
        chosen_path: file.to_string_lossy().into_owned(),
        engine: "laya".into(),
        p: Some(0.9),
    });
    s.flush();
    let h = host(vec![hit("other.pdf", 300)]);
    assert_eq!(run(&h, &s, &[]).await, Outcome::HideLauncher);
    assert_eq!(opened(&h), ["cached.pdf"]);
    assert!(h.queries.lock().is_empty());
}

#[tokio::test]
async fn auto_open_is_cached() {
    let h = host(vec![hit("tax-invoice.pdf", 300)]);
    let s = Store::open_in_memory().unwrap();
    run(&h, &s, &[]).await;
    s.flush();
    let c = s.nl_cache_get(&cache_key(PHRASE)).unwrap().unwrap();
    assert_eq!(c.engine, "parser");
}

#[tokio::test]
async fn roots_restrict_the_query() {
    let h = host(vec![]);
    let s = Store::open_in_memory().unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    resolve(
        h.clone(),
        &s,
        "invoice pdf in downloads folder",
        today,
        &[],
        0.6,
        8,
    )
    .await
    .unwrap();
    assert_eq!(h.queries.lock()[0].root, Some(2));
}

fn input(text: &str, mode: Mode) -> Input {
    Input {
        raw: text.into(),
        text: text.into(),
        mode,
        seq: 1,
    }
}

#[test]
fn query_is_one_cheap_item_for_natural_find_input_only() {
    let f = NlFind::new(host(vec![]));
    let got = f.query(&input(PHRASE, Mode::Find));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].title, format!("Find: {PHRASE}"));
    assert!(f.query(&input("report", Mode::Find)).is_empty());
    assert!(f.query(&input(PHRASE, Mode::Command)).is_empty());
    let t = std::time::Instant::now();
    for _ in 0..100 {
        f.query(&input(PHRASE, Mode::Find));
    }
    assert!(t.elapsed() / 100 < Duration::from_micros(500));
}
