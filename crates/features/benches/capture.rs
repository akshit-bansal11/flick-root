//! Query bench for capture; budget 0.5 ms.

use arc_swap::ArcSwap;
use criterion::{Criterion, criterion_group, criterion_main};
use features::capture::Capture;
use features::{EventSink, Feature, FeatureCtx, TaskScope};
use launcher_core::store::Store;
use launcher_core::{Config, Input, Mode};
use providers::hub::ProviderHub;
use std::hint::black_box;
use std::sync::Arc;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

fn bench(c: &mut Criterion) {
    let mut cfg = Config::default();
    cfg.integrations.markdown_file = "C:/bench/inbox.md".into();
    let f = Capture::default();
    f.start(FeatureCtx {
        config: Arc::new(ArcSwap::from_pointee(cfg)),
        store: Arc::new(Store::open_in_memory().expect("store")),
        providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
        http: reqwest::Client::new(),
        events: Arc::new(Sink),
        tasks: TaskScope::new(),
    })
    .expect("start");
    for q in [
        "log 250 lunch #food",
        concat!("log my key sk-a", "B3xK9mQaB3xK9mQaB3xK9mQaB3xK9mQaB3xK9mQ"),
        "hello",
    ] {
        let input = Input {
            raw: q.into(),
            text: q.into(),
            mode: Mode::Command,
            seq: 1,
        };
        c.bench_function(&format!("capture/query/{q}"), |b| {
            b.iter(|| black_box(f.query(black_box(&input))))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
