//! Query bench for nlfind; budget 0.5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use features::Feature;
use features::{NlFind, NlHost};
use index_files::{FileHit, FindQuery};
use secrecy::SecretString;
use std::path::Path;
use std::sync::Arc;

struct Host;
impl NlHost for Host {
    fn find(&self, _: &FindQuery) -> Vec<FileHit> {
        Vec::new()
    }
    fn root_id(&self, _: &str) -> Option<u8> {
        None
    }
    fn open(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
    fn engine_key(&self, _: &str, _: &str) -> Option<SecretString> {
        None
    }
}
use launcher_core::{Input, Mode};
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let f = NlFind::new(Arc::new(Host));
    for q in ["pdf from yesterday", "report", "a"] {
        let input = Input {
            raw: q.into(),
            text: q.into(),
            mode: Mode::Find,
            seq: 1,
        };
        c.bench_function(&format!("nlfind/query/{q}"), |b| {
            b.iter(|| black_box(f.query(black_box(&input))))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
