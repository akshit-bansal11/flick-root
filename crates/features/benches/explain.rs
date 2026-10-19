//! Bench for explain. Its query() returns nothing, so this times the secret check that guards
//! the capture (`analyze` on a 2,000-char selection); budget 0.5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use features::explain::analyze;
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let plain = "The quick brown fox jumps over the lazy dog. ".repeat(50);
    let secret = format!("{} sk-{}", &plain[..1900], "aB3xK9mQ".repeat(5));
    for (name, text) in [("plain", plain), ("secret", secret)] {
        c.bench_function(&format!("explain/analyze/{name}"), |b| {
            b.iter(|| black_box(analyze(black_box(Some(&text)), "notepad.exe")))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
