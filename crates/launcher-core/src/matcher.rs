//! Fuzzy matcher wrapper around nucleo-matcher. Haystacks are converted once
//! into a [`MatchIndex`]; each keystroke parses the pattern once and keeps the
//! best `k` hits in a small heap.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str, Utf32String};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// One scored hit: index into the haystack list and nucleo's score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scored {
    /// Index of the haystack.
    pub idx: usize,
    /// Match score, higher is better.
    pub score: u32,
}

/// Haystacks pre-converted to UTF-32, built when the list changes.
#[derive(Debug, Default)]
pub struct MatchIndex {
    items: Vec<Utf32String>,
}

impl MatchIndex {
    /// Converts every haystack once.
    pub fn new<S: AsRef<str>>(haystacks: &[S]) -> Self {
        Self {
            items: haystacks
                .iter()
                .map(|s| Utf32String::from(s.as_ref()))
                .collect(),
        }
    }

    /// Number of haystacks.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when there are no haystacks.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterates `(idx, haystack)` pairs, for [`FuzzyMatcher::top_k_iter`].
    pub fn iter(&self) -> impl Iterator<Item = (usize, Utf32Str<'_>)> {
        self.items.iter().enumerate().map(|(i, s)| (i, s.slice(..)))
    }
}

/// Reusable matcher that keeps nucleo's scratch buffers between calls.
pub struct FuzzyMatcher {
    matcher: Matcher,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyMatcher {
    /// Matcher for app and command names (prefix matches preferred).
    pub fn new() -> Self {
        let mut cfg = Config::DEFAULT;
        cfg.prefer_prefix = true;
        Self {
            matcher: Matcher::new(cfg),
        }
    }

    /// Matcher tuned for file paths (Find mode).
    pub fn new_paths() -> Self {
        let mut cfg = Config::DEFAULT.match_paths();
        cfg.prefer_prefix = true;
        Self {
            matcher: Matcher::new(cfg),
        }
    }

    /// Score of `query` matched against itself: the ceiling a haystack hit can reach, so callers
    /// can scale scores on one absolute scale. Returns 0 for an empty query.
    pub fn perfect_score(&mut self, query: &str) -> u32 {
        if query.is_empty() {
            return 0;
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let hay = Utf32String::from(query);
        pattern.score(hay.slice(..), &mut self.matcher).unwrap_or(0)
    }

    /// Best `k` hits for `query`, sorted by score descending (ties: lower idx
    /// first). An empty query returns nothing; callers show most-used.
    pub fn top_k(&mut self, query: &str, index: &MatchIndex, k: usize) -> Vec<Scored> {
        self.top_k_iter(query, index.iter(), k)
    }

    /// Same as [`top_k`](Self::top_k) for callers that own their haystacks.
    pub fn top_k_iter<'a>(
        &mut self,
        query: &str,
        haystacks: impl Iterator<Item = (usize, Utf32Str<'a>)>,
        k: usize,
    ) -> Vec<Scored> {
        if query.is_empty() || k == 0 {
            return Vec::new();
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        // Min-heap on (score, lower idx is better): the worst hit sits on top.
        let mut heap: BinaryHeap<Reverse<(u32, Reverse<usize>)>> = BinaryHeap::with_capacity(k + 1);
        for (idx, hay) in haystacks {
            let Some(score) = pattern.score(hay, &mut self.matcher) else {
                continue;
            };
            heap.push(Reverse((score, Reverse(idx))));
            if heap.len() > k {
                heap.pop();
            }
        }
        // into_sorted_vec ascends by Reverse(..), i.e. best first.
        heap.into_sorted_vec()
            .into_iter()
            .map(|Reverse((score, Reverse(idx)))| Scored { idx, score })
            .collect()
    }
}

/// True when `a` and `b` are within one Damerau-Levenshtein edit (insert, delete, substitute or
/// adjacent swap), case-insensitively. Equal strings count as within one edit.
pub fn one_edit(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().flat_map(char::to_lowercase).collect();
    let b: Vec<char> = b.chars().flat_map(char::to_lowercase).collect();
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[pre..], &b[pre..]);
    let suf = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a, b) = (&a[..a.len() - suf], &b[..b.len() - suf]);
    match (a.len(), b.len()) {
        (0, 0) | (1, 0) | (0, 1) | (1, 1) => true,
        (2, 2) => a[0] == b[1] && a[1] == b[0],
        _ => false,
    }
}

fn is_subsequence(q: &str, hay: impl Iterator<Item = char>) -> bool {
    let mut q = q.chars();
    let mut next = q.next();
    for c in hay {
        if next == Some(c) {
            next = q.next();
        }
    }
    next.is_none()
}

/// Fuzzy tier of one query word against one keyword word: prefix or subsequence 0.45, one edit 0.4.
fn word_fuzzy(q: &str, w: &str) -> Option<f32> {
    if w.starts_with(q) {
        return Some(0.45);
    }
    if q.chars().count() < 3 {
        return None;
    }
    if is_subsequence(q, w.chars()) {
        Some(0.45)
    } else if one_edit(q, w) {
        Some(0.4)
    } else {
        None
    }
}

/// Scores a query against one keyword (alias): exact 1.0, prefix 0.95, a later word's prefix 0.6,
/// then fuzzy tiers (subsequence 0.45, one edit 0.4) that need a query of at least 3 characters.
/// A multi-word query must match the keyword's words one by one; the worst word sets the score.
pub fn keyword_score(query: &str, keyword: &str) -> Option<f32> {
    let q = query.trim().to_lowercase();
    let k = keyword.to_lowercase();
    if q.is_empty() {
        return None;
    }
    if q == k {
        return Some(1.0);
    }
    if k.starts_with(&q) {
        return Some(0.95);
    }
    if k.split(' ').skip(1).any(|w| w.starts_with(&q)) {
        return Some(0.6);
    }
    if q.chars().count() < 3 {
        return None;
    }
    let mut words = q.split_whitespace();
    let (first, second) = (words.next()?, words.next());
    if second.is_none() {
        // Single word: also try the keyword with its spaces removed ("shtdwn" in "shut down").
        if is_subsequence(first, k.chars().filter(|c| *c != ' ')) {
            return Some(0.45);
        }
        return k
            .split(' ')
            .filter_map(|w| word_fuzzy(first, w))
            .reduce(f32::max);
    }
    q.split_whitespace()
        .map(|qw| {
            k.split(' ')
                .filter_map(|w| word_fuzzy(qw, w))
                .reduce(f32::max)
        })
        .try_fold(1.0f32, |acc, s| s.map(|s| acc.min(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_score_tiers() {
        let cases: [(&str, &str, Option<f32>); 10] = [
            ("lock", "lock", Some(1.0)),
            ("loc", "lock", Some(0.95)),
            ("off", "bluetooth off", Some(0.6)),
            ("slep", "sleep", Some(0.45)),
            ("shtdwn", "shut down", Some(0.45)),
            ("sleeo", "sleep", Some(0.4)),
            ("lcok", "lock", Some(0.4)),
            ("blutooth off", "bluetooth off", Some(0.45)),
            ("sl", "lock", None),
            ("xyz", "lock", None),
        ];
        for (q, k, want) in cases {
            assert_eq!(keyword_score(q, k), want, "{q} vs {k}");
        }
    }

    #[test]
    fn one_edit_table() {
        for (a, b, want) in [
            ("lcok", "lock", true),
            ("lock", "locks", true),
            ("locks", "lock", true),
            ("lack", "lock", true),
            ("lock", "LOCK", true),
            ("lock", "lock", true),
            ("lcokk", "lock", false),
            ("abc", "xyz", false),
            ("ab", "abcd", false),
        ] {
            assert_eq!(one_edit(a, b), want, "{a} vs {b}");
        }
    }

    #[test]
    fn empty_query_is_empty() {
        let idx = MatchIndex::new(&["Code"]);
        assert!(FuzzyMatcher::new().top_k("", &idx, 5).is_empty());
    }

    #[test]
    fn top_k_sorted_and_capped() {
        let idx = MatchIndex::new(&[
            "Terminal",
            "Code",
            "Codec Tool",
            "Notepad",
            "Visual Studio Code",
        ]);
        let hits = FuzzyMatcher::new().top_k("code", &idx, 2);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score >= hits[1].score);
        assert!(hits.iter().all(|h| h.idx != 0 && h.idx != 3));
        assert_eq!(hits[0].idx, 1, "exact full-name match wins");
    }

    #[test]
    fn perfect_score_bounds_every_hit() {
        let idx = MatchIndex::new(&[
            "inlay-2.0.0",
            "windows media player legacy",
            "visual studio code",
            "code",
            "spotify",
        ]);
        let mut m = FuzzyMatcher::new();
        assert_eq!(m.perfect_score(""), 0);
        for q in ["inlay", "code", "spot", "media"] {
            let perfect = m.perfect_score(q);
            assert!(perfect > 0, "{q}");
            for h in m.top_k(q, &idx, 5) {
                assert!(h.score <= perfect, "{q}: {} > {perfect}", h.score);
            }
        }
    }

    #[test]
    fn iter_variant_matches_index_variant() {
        let idx = MatchIndex::new(&["Spotify", "Spot", "Photoshop"]);
        let mut m = FuzzyMatcher::new();
        assert_eq!(
            m.top_k("spot", &idx, 3),
            m.top_k_iter("spot", idx.iter(), 3)
        );
    }
}
