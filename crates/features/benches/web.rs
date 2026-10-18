//! Bench of `query()` for web shortcuts (budget 0.5 ms).

use criterion::{Criterion, criterion_group, criterion_main};
use features::Feature;
use features::web::Web;
use launcher_core::{Input, Mode};
use std::hint::black_box;

fn input(text: &str) -> Input {
    Input {
        raw: text.into(),
        text: text.into(),
        mode: Mode::Command,
        seq: 1,
    }
}

fn bench(c: &mut Criterion) {
    let hit = input("yt lofi beats");
    let miss = input("notepad");
    c.bench_function("web_query_hit", |b| b.iter(|| Web.query(black_box(&hit))));
    c.bench_function("web_query_miss", |b| b.iter(|| Web.query(black_box(&miss))));
}

criterion_group!(benches, bench);
criterion_main!(benches);
