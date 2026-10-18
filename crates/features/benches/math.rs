//! Query latency for the math feature (budget 0.5 ms).

use criterion::{Criterion, criterion_group, criterion_main};
use features::{Feature, math::Math};
use launcher_core::{Input, Mode};
use std::hint::black_box;

fn input(t: &str) -> Input {
    Input {
        raw: t.into(),
        text: t.into(),
        mode: Mode::Command,
        seq: 1,
    }
}

fn bench(c: &mut Criterion) {
    for t in ["2.5 kg to lb", "5pm ist in pst", "$20 in inr", "notepad"] {
        let i = input(t);
        c.bench_function(&format!("math query {t}"), |b| {
            b.iter(|| Math.query(black_box(&i)))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
