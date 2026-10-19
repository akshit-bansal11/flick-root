use super::parse::Kind;
use super::*;
use crate::ctx::{EventSink, TaskScope};
use arc_swap::ArcSwap;
use launcher_core::config::Webhook;
use launcher_core::store::Store;
use providers::hub::ProviderHub;
use providers::secret::{MemoryBackend, MemoryMeta, SecretName};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

fn vault() -> Arc<SecretVault> {
    Arc::new(SecretVault::new(
        Arc::new(MemoryBackend::default()),
        Arc::new(MemoryMeta::default()),
    ))
}

fn started(cap: &Capture, cfg: Config) {
    cap.start(FeatureCtx {
        config: Arc::new(ArcSwap::from_pointee(cfg)),
        store: Arc::new(Store::open_in_memory().unwrap()),
        providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
        http: reqwest::Client::new(),
        events: Arc::new(Sink),
        tasks: TaskScope::new(),
    })
    .unwrap();
}

fn input(t: &str) -> Input {
    Input {
        raw: t.into(),
        text: t.into(),
        mode: Mode::Command,
        seq: 1,
    }
}

fn primary() -> Action {
    Action {
        kind: ActionKind::Primary,
        confirm_token: None,
    }
}

fn md_cfg(file: &Path) -> Config {
    let mut c = Config::default();
    c.integrations.markdown_file = file.to_string_lossy().into_owned();
    c
}

#[test]
fn unrelated_or_empty_input_gives_no_rows() {
    let cap = Capture::default();
    started(&cap, Config::default());
    for t in ["hello", "log", "log   ", "logbook 5"] {
        assert!(cap.query(&input(t)).is_empty(), "{t}");
    }
}

#[test]
fn without_a_target_one_setup_row_opens_settings() {
    let cap = Capture::default();
    started(&cap, Config::default());
    let rows = cap.query(&input("log 250 lunch"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Set up quick capture");
    assert_eq!(rows[0].payload["open_settings"], "integrations");
}

#[test]
fn one_row_per_configured_target_with_a_preview() {
    let d = tempfile::tempdir().unwrap();
    let mut c = md_cfg(&d.path().join("inbox.md"));
    c.integrations.obsidian.vault = d.path().to_string_lossy().into_owned();
    c.integrations.notion.database_id = "db".into();
    c.integrations.webhooks = vec![
        Webhook {
            name: "money".into(),
            enabled: true,
            host: "h.test".into(),
        },
        Webhook {
            name: "off".into(),
            enabled: false,
            host: "h.test".into(),
        },
    ];
    let cap = Capture::default();
    started(&cap, c);
    let rows = cap.query(&input("log 250 lunch #food"));
    let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Add to Obsidian daily note",
            "Append to inbox.md",
            "Add to Notion",
            "Send to money"
        ]
    );
    assert!(rows.iter().all(|r| r.subtitle == "250 · lunch · #food"));
}

#[tokio::test]
async fn secret_text_is_blocked_in_rows_and_in_execute() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("inbox.md");
    let cap = Capture::default();
    started(&cap, md_cfg(&file));
    let key = format!("sk-{}", "aB3xK9mQ".repeat(5));
    let rows = cap.query(&input(&format!("note my key {key}")));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Blocked: looks like a secret");
    assert!(rows[0].actions.is_empty());
    assert!(!rows[0].subtitle.contains(&key));
    // A forged row for a real target is refused too.
    let mut forged = cap.query(&input("note hi"))[0].clone();
    forged.payload["raw"] = json!(format!("note {key}"));
    let r = cap.execute(&forged, primary()).await;
    assert!(matches!(r, Err(FeatureError::SecretBlocked(_))));
    assert!(!file.exists());
}

#[tokio::test]
async fn execute_appends_to_the_markdown_file() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("inbox.md");
    let cap = Capture::default();
    started(&cap, md_cfg(&file));
    let row = cap.query(&input("log 250 lunch #food"))[0].clone();
    let out = cap.execute(&row, primary()).await.unwrap();
    assert_eq!(out, Outcome::HideLauncher);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.ends_with("lunch (250) #food\n"), "{text}");
    assert!(text.starts_with("- "));
}

#[tokio::test]
async fn unclear_log_without_a_provider_captures_the_raw_text() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("inbox.md");
    let cap = Capture::default();
    started(&cap, md_cfg(&file));
    let row = cap.query(&input("log lunch with ana"))[0].clone();
    cap.execute(&row, primary()).await.unwrap();
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("lunch with ana")
    );
}

#[tokio::test]
async fn stopped_feature_has_no_rows_and_refuses() {
    let d = tempfile::tempdir().unwrap();
    let cap = Capture::default();
    started(&cap, md_cfg(&d.path().join("a.md")));
    let row = cap.query(&input("note hi"))[0].clone();
    cap.stop();
    assert!(cap.query(&input("note hi")).is_empty());
    assert!(cap.execute(&row, primary()).await.is_err());
}

#[tokio::test]
async fn webhook_gets_text_fields_and_time() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&s)
        .await;
    let v = vault();
    let n = SecretName::parse("integration.webhook.ci@127.0.0.1").unwrap();
    v.set(&n, format!("{}/hook/abc", s.uri()).into()).unwrap();
    let mut c = Config::default();
    c.integrations.webhooks = vec![Webhook {
        name: "ci".into(),
        enabled: true,
        host: "127.0.0.1".into(),
    }];
    let cap = Capture::new(v);
    started(&cap, c);
    let row = cap.query(&input("log 250 lunch"))[0].clone();
    cap.execute(&row, primary()).await.unwrap();
    let req = &s.received_requests().await.unwrap()[0];
    let b: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(b["text"], "lunch");
    assert_eq!(b["fields"]["amount"], 250.0);
    assert!(b["time"].is_string());
}

#[tokio::test]
async fn test_target_reports_not_configured_and_checks_files() {
    let d = tempfile::tempdir().unwrap();
    let v = vault();
    let http = reqwest::Client::new();
    let mut c = md_cfg(&d.path().join("a.md"));
    assert!(test_target(&v, &c, &http, "markdown").await.is_ok());
    c.integrations.webhooks = vec![Webhook {
        name: "ci".into(),
        enabled: true,
        host: "h.test".into(),
    }];
    let e = test_target(&v, &c, &http, "webhook.ci").await.unwrap_err();
    assert_eq!(e.code(), "not_configured");
    let e = test_target(&v, &c, &http, "notion").await.unwrap_err();
    assert_eq!(e.code(), "not_configured");
}

#[test]
fn model_reply_is_parsed_strictly() {
    let ok = |s| parse_fill(s).is_some();
    assert!(ok(
        r#"{"amount": 12.5, "tags": ["food"], "date": "2026-10-19"}"#
    ));
    assert!(ok(
        "```json\n{\"amount\":null,\"tags\":[],\"date\":null}\n```"
    ));
    assert!(!ok("Sure: {\"amount\":1,\"tags\":[],\"date\":null}"));
    assert!(!ok(
        r#"{"amount":1,"tags":[],"date":null,"target":"notion"}"#
    ));
    assert!(!ok(r#"{"amount":-5,"tags":[],"date":null}"#));
    assert!(!ok(r#"{"amount":1,"tags":["a b"],"date":null}"#));
    assert!(!ok(r#"{"amount":1,"tags":[],"date":"tomorrow"}"#));
}

#[test]
fn fill_never_overwrites_parsed_fields() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    let mut p = parse::parse(Kind::Log, "lunch 250", today);
    let f = parse_fill(r#"{"amount": 999, "tags": ["Food"], "date": "2026-01-01"}"#).unwrap();
    apply_fill(&mut p, f);
    assert_eq!(p.amount, Some(250.0));
    assert_eq!(p.tags, ["food"]);
    assert_eq!(p.date, NaiveDate::from_ymd_opt(2026, 1, 1));
}

#[test]
fn query_is_cheap() {
    let d = tempfile::tempdir().unwrap();
    let mut c = md_cfg(&d.path().join("a.md"));
    c.integrations.notion.database_id = "db".into();
    let cap = Capture::default();
    started(&cap, c);
    let i = input("log 250 lunch #food yesterday 12:30");
    let t = std::time::Instant::now();
    for _ in 0..200 {
        std::hint::black_box(cap.query(&i));
    }
    let per = t.elapsed() / 200;
    assert!(per < Duration::from_micros(500), "{per:?}");
}
