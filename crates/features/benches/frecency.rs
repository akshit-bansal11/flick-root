//! rank_boost over 2,000 items with a 5,000-entry history: one query's total boost cost.

use arc_swap::ArcSwap;
use criterion::{Criterion, criterion_group, criterion_main};
use features::frecency::Frecency;
use features::{EventSink, Feature, FeatureCtx, TaskScope};
use launcher_core::store::Store;
use launcher_core::{Config, Input, Item, ItemId, ItemKind, Mode};
use providers::hub::ProviderHub;
use std::hint::black_box;
use std::sync::Arc;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

fn item(i: usize) -> Item {
    Item {
        id: ItemId(format!("app:{i}")),
        kind: ItemKind::App,
        title: format!("App {i}"),
        subtitle: String::new(),
        icon: None,
        score: 1.0,
        source: "apps".into(),
        actions: vec![],
        payload: serde_json::Value::Null,
    }
}

fn bench(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let _g = rt.enter();
    let store = Arc::new(Store::open_in_memory().expect("store"));
    for i in 0..5000 {
        store.record_launch(&format!("app:{i}"), "app", None, 1_700_000_000_000);
    }
    store.flush();
    let ctx = FeatureCtx {
        config: Arc::new(ArcSwap::from_pointee(Config::default())),
        store,
        providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
        http: reqwest::Client::new(),
        events: Arc::new(Sink),
        tasks: TaskScope::new(),
    };
    let f = Frecency::default();
    f.start(ctx).expect("start");
    let items: Vec<Item> = (0..2000).map(item).collect();
    let input = Input {
        raw: "app".into(),
        text: "app".into(),
        mode: Mode::Command,
        seq: 1,
    };
    // Let the background load finish.
    rt.block_on(tokio::time::sleep(std::time::Duration::from_millis(500)));
    c.bench_function("rank_boost_2000_items_5000_history", |b| {
        b.iter(|| {
            items
                .iter()
                .map(|it| f.rank_boost(black_box(&input), it))
                .sum::<f32>()
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
