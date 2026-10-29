//! Command search over 2,000 apps with every built-in feature running (budget 2 ms per query),
//! and the separate programs match over 50,000 synthetic names (budget 25 ms).

use arc_swap::ArcSwap;
use criterion::{Criterion, criterion_group, criterion_main};
use features::math::Math;
use features::search::{program_index, program_items};
use features::{EventSink, Feature, FeatureCtx, Registry, TaskScope, command_search};
use index_apps::AppEntry;
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use launcher_core::store::{RateRow, Store};
use launcher_core::{Config, Input, Mode};
use providers::hub::ProviderHub;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

fn input(t: &str) -> Input {
    Input {
        raw: t.into(),
        text: t.into(),
        mode: Mode::Command,
        seq: 1,
    }
}

fn app(i: usize, name: String) -> AppEntry {
    AppEntry {
        id: format!("id{i}"),
        name,
        parsing_name: format!("p.{i}"),
        target: None,
        is_packaged: false,
    }
}

fn apps() -> Vec<AppEntry> {
    let base = [
        "Visual Studio Code",
        "Microsoft Edge",
        "Calculator",
        "Notepad",
        "Windows Terminal",
        "File Explorer",
        "Google Chrome",
        "Mozilla Firefox",
        "Slack",
        "Discord",
    ];
    (0..2000)
        .map(|i| {
            let n = if i < base.len() {
                base[i].to_string()
            } else {
                format!("{} {}", base[i % base.len()], i)
            };
            app(i, n)
        })
        .collect()
}

/// Rate rows from the ECB fixture; `fetched_at` is now so no refresh is wanted.
fn rate_rows() -> Vec<RateRow> {
    let xml = include_str!("../tests/fixtures/eurofxref-daily.xml");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    xml.split("<Cube currency='")
        .skip(1)
        .filter_map(|s| {
            let (cur, rest) = s.split_once("' rate='")?;
            let (rate, _) = rest.split_once('\'')?;
            Some(RateRow {
                currency: cur.into(),
                per_eur: rate.parse().ok()?,
                rate_date: "2026-10-16".into(),
                fetched_at: now,
            })
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("rt");
    let apps = apps();
    let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
    let idx = MatchIndex::new(&names);
    let mut matcher = FuzzyMatcher::new();

    let store = Arc::new(Store::open_in_memory().expect("store"));
    for i in 0..300 {
        store.record_launch(&format!("app:p.{i}"), "app", None, 1_700_000_000_000);
    }
    store.rates_upsert(rate_rows());
    store.flush();

    let mut cfg = Config::default();
    let mut m = toml::Table::new();
    m.insert("currency_refresh".into(), toml::Value::Boolean(false));
    cfg.features.insert("math".into(), m);
    let ctx = FeatureCtx {
        config: Arc::new(ArcSwap::from_pointee(cfg.clone())),
        store,
        providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
        http: reqwest::Client::new(),
        events: Arc::new(Sink),
        tasks: TaskScope::new(),
    };
    features::math::warm();
    let reg = Registry::new();
    rt.block_on(async {
        for f in features::builtin_features() {
            reg.register(f).await;
        }
        reg.apply_config(&cfg, &ctx).await;
    });
    // Wait for the frecency load and the rates cache to publish.
    let ready = |t: &str| {
        let i = input(t);
        !Math.query(&i).is_empty()
    };
    for _ in 0..500 {
        if ready("250 usd in inr") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ready("250 usd in inr"), "rates never published");

    for t in [
        "c",
        "co",
        "cod",
        "code",
        "lock",
        "g cats",
        "2.5 kg to lb",
        "250 usd in inr",
        "5pm ist in pst",
        "notepad",
        "zzzz",
        "spotfy",
        "port 3000",
        "proc chrome",
        "play lofi on spotify",
        "log 250 lunch #food",
    ] {
        let i = input(t);
        c.bench_function(&format!("command_all/{t}"), |b| {
            b.iter(|| command_search(&apps, &idx, &mut matcher, &reg, black_box(&i), 8))
        });
    }
    // Programs live in their own index and are matched off the keystroke thread (Find's 25 ms budget).
    let stems = [
        "setup", "install", "update", "launcher", "tool", "driver", "game", "helper",
    ];
    let programs: Vec<Box<str>> = (0..50_000)
        .map(|i| format!(r"C:\p\{} {} v{}.exe", stems[i % stems.len()], i, i % 97).into_boxed_str())
        .collect();
    let pidx = program_index(&programs);
    let mut pmatcher = FuzzyMatcher::new();
    for t in ["c", "co", "setup", "inst 4", "zzzz"] {
        c.bench_function(&format!("programs_50k/{t}"), |b| {
            b.iter(|| program_items(&programs, &[], &pidx, &mut pmatcher, black_box(t), 8))
        });
    }
    // The apps part alone, with 50k programs present in memory but not in the apps index.
    let i = input("code");
    c.bench_function("apps_part_2k/code", |b| {
        b.iter(|| command_search(&apps, &idx, &mut matcher, &reg, black_box(&i), 8))
    });
    let cur = input("250 usd in inr");
    c.bench_function("math_query/250 usd in inr (rates loaded)", |b| {
        b.iter(|| Math.query(black_box(&cur)))
    });
    rt.block_on(reg.stop_all());
}

criterion_group!(benches, bench);
criterion_main!(benches);
