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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn iter_variant_matches_index_variant() {
        let idx = MatchIndex::new(&["Spotify", "Spot", "Photoshop"]);
        let mut m = FuzzyMatcher::new();
        assert_eq!(
            m.top_k("spot", &idx, 3),
            m.top_k_iter("spot", idx.iter(), 3)
        );
    }
}
