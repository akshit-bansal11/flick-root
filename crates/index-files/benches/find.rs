//! Find benchmark over 300,000 synthetic paths; prints p95 per query (budget 25 ms).

use criterion::{Criterion, criterion_group, criterion_main};
use index_files::store::{NO_PARENT, StoreBuilder};
use index_files::{FileStore, FindQuery, Overlay, Root, query};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap_or(0) % n
    }
}

const WORDS: [&str; 16] = [
    "report",
    "invoice",
    "budget",
    "photo",
    "notes",
    "draft",
    "summary",
    "meeting",
    "design",
    "backup",
    "screenshot",
    "resume",
    "contract",
    "plan",
    "data",
    "archive",
];
const EXTS: [&str; 10] = [
    "pdf", "docx", "xlsx", "png", "jpg", "txt", "md", "zip", "mp4", "rs",
];

fn build() -> FileStore {
    let mut r = Lcg(42);
    let mut b = StoreBuilder::new();
    let mut roots = Vec::new();
    let mut total = 0usize;
    'outer: for id in 0u8..6 {
        let path = PathBuf::from(format!("C:\\Users\\bench\\Root{id}"));
        let Some(root) = b.push(&path.to_string_lossy(), NO_PARENT, true, 0, id) else {
            continue;
        };
        roots.push(Root {
            id,
            path,
            label: format!("Root{id}"),
        });
        for year in 2015..2026 {
            let Some(y) = b.push(&year.to_string(), root, true, 1, id) else {
                continue;
            };
            for month in 1..=12 {
                let Some(m) = b.push(&format!("{month:02}"), y, true, 1, id) else {
                    continue;
                };
                for p in 0..6 {
                    let proj = format!("project_{}_{p}", WORDS[r.next(WORDS.len())]);
                    let Some(pi) = b.push(&proj, m, true, 1, id) else {
                        continue;
                    };
                    for f in 0..64 {
                        let name = format!(
                            "{}_{}_{}.{}",
                            WORDS[r.next(WORDS.len())],
                            year,
                            f,
                            EXTS[r.next(EXTS.len())]
                        );
                        let min = u32::try_from(r.next(30_000_000)).unwrap_or(0);
                        b.push(&name, pi, false, min, id);
                        total += 1;
                    }
                    if total >= 300_000 {
                        break 'outer;
                    }
                }
            }
        }
    }
    b.finish(roots)
}

fn report(c: &mut Criterion, store: &FileStore, ov: &Overlay, label: &str) {
    for text in ["rep", "report", "inv 2024", "png", "budget xlsx", "a"] {
        let q = FindQuery {
            text: text.into(),
            limit: 50,
            ..Default::default()
        };
        let mut t: Vec<f64> = (0..200)
            .map(|_| {
                let s = Instant::now();
                black_box(query(store, ov, &q));
                s.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        t.sort_by(f64::total_cmp);
        println!("p95 [{label}] {text:>12}: {:.2} ms (budget 25)", t[189]);
        c.bench_function(&format!("find/{label}/{text}"), |b| {
            b.iter(|| black_box(query(store, ov, &q)));
        });
    }
}

fn bench(c: &mut Criterion) {
    let store = build();
    println!("store entries: {}", store.len());
    report(c, &store, &Overlay::new(), "empty");

    // Non-empty overlay: 1,000 removals spread over the tree plus 1,000 real adds.
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut roots = store.roots().to_vec();
    roots.push(Root {
        id: 6,
        path: tmp.path().to_path_buf(),
        label: "tmp".into(),
    });
    let ov = Overlay::new();
    let mut r = Lcg(7);
    for _ in 0..1000 {
        ov.apply_path(&roots, &store.path_of(r.next(store.len())));
    }
    for n in 0..1000 {
        let p = tmp.path().join(format!("report_added_{n}.txt"));
        std::fs::write(&p, "x").expect("write");
        ov.apply_path(&roots, &p);
    }
    println!("overlay entries: {}", ov.len());
    report(c, &store, &ov, "overlay");
}

criterion_group!(benches, bench);
criterion_main!(benches);
