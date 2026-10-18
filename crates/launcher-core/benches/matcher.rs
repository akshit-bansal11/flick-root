//! Command match budget: top_k(k=8) over 2,000 app names <= 2 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use std::hint::black_box;

const WORDS: [&str; 64] = [
    "Microsoft",
    "Visual",
    "Studio",
    "Code",
    "Adobe",
    "Photoshop",
    "Spotify",
    "Terminal",
    "Chrome",
    "Firefox",
    "Edge",
    "Notepad",
    "Calculator",
    "Paint",
    "Word",
    "Excel",
    "PowerPoint",
    "Outlook",
    "Teams",
    "Slack",
    "Discord",
    "Steam",
    "Epic",
    "Games",
    "Launcher",
    "Git",
    "Bash",
    "Docker",
    "Desktop",
    "Node",
    "Python",
    "Rust",
    "Cargo",
    "Java",
    "Eclipse",
    "IntelliJ",
    "Android",
    "Emulator",
    "Blender",
    "Unity",
    "Hub",
    "Editor",
    "Player",
    "Media",
    "VLC",
    "Zoom",
    "Notion",
    "Figma",
    "Postman",
    "Insomnia",
    "Wireshark",
    "Putty",
    "WinSCP",
    "Explorer",
    "Settings",
    "Control",
    "Panel",
    "Task",
    "Manager",
    "Snipping",
    "Tool",
    "Camera",
    "Photos",
    "Mail",
];

/// Deterministic names from a simple LCG, 1 to 4 words each.
fn names(n: usize) -> Vec<String> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    (0..n)
        .map(|_| {
            let count = 1 + next() % 4;
            let parts: Vec<&str> = (0..count).map(|_| WORDS[next() % WORDS.len()]).collect();
            parts.join(" ")
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let index = MatchIndex::new(&names(2000));
    let mut m = FuzzyMatcher::new();
    for q in ["c", "co", "cod", "code", "vsc", "photo", "spot", "termin"] {
        c.bench_function(&format!("top_k_8/{q}"), |b| {
            b.iter(|| black_box(m.top_k(black_box(q), &index, 8)))
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
