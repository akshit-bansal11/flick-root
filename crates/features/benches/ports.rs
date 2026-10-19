//! Query bench for ports; budget 0.5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use features::Feature;
use features::ports::Ports;
use launcher_core::{Input, Mode};
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let f = Ports::default();
    for q in ["port 3000", "proc chrome", "hello"] {
        let input = Input {
            raw: q.into(),
            text: q.into(),
            mode: Mode::Command,
            seq: 1,
        };
        c.bench_function(&format!("ports/query/{q}"), |b| {
            b.iter(|| black_box(f.query(black_box(&input))))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
