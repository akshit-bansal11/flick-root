//! Bench of `query()` over ~500 scanned projects (budget 0.5 ms).

use arc_swap::ArcSwap;
use criterion::{Criterion, criterion_group, criterion_main};
use features::projects::Projects;
use features::{EventSink, Feature, FeatureCtx, TaskScope};
use launcher_core::store::Store;
use launcher_core::{Config, Input, Mode};
use providers::hub::ProviderHub;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

fn bench(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..500 {
        let p = dir.path().join(format!("proj-{i:03}-service"));
        std::fs::create_dir_all(&p).expect("mkdir");
        std::fs::write(p.join("Cargo.toml"), "").expect("write");
    }
    let mut cfg = Config::default();
    let mut t = toml::Table::new();
    t.insert(
        "roots".into(),
        toml::Value::Array(vec![dir.path().to_string_lossy().into_owned().into()]),
    );
    cfg.features.insert("projects".into(), t);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
    let f = Projects::default();
    {
        let _g = rt.enter();
        f.start(FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(cfg)),
            store: Arc::new(Store::open_in_memory().expect("store")),
            providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        })
        .expect("start");
    }
    let q = Input {
        raw: "serv 42".into(),
        text: "serv 42".into(),
        mode: Mode::Command,
        seq: 1,
    };
    while f.query(&q).is_empty() {
        std::thread::sleep(Duration::from_millis(10));
    }
    c.bench_function("projects_query_500", |b| b.iter(|| f.query(black_box(&q))));
}

criterion_group!(benches, bench);
criterion_main!(benches);
