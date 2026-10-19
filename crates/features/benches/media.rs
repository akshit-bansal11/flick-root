//! Query bench for media; budget 0.5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use features::Feature;
use features::media::Media;
use launcher_core::{Input, Mode};
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let f = Media::default();
    for q in ["play", "play lofi on spotify", "hello"] {
        let input = Input {
            raw: q.into(),
            text: q.into(),
            mode: Mode::Command,
            seq: 1,
        };
        c.bench_function(&format!("media/query/{q}"), |b| {
            b.iter(|| black_box(f.query(black_box(&input))))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
