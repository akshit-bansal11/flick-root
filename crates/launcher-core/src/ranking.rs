//! Ranking hooks and score blending.

use crate::types::Item;

/// Extra score for an item, e.g. from usage history. Called per item per query.
pub trait RankHook: Send + Sync {
    /// Boost added to the normalized match score (0..1 scale). May be negative.
    fn boost(&self, query: &str, item_id: &str) -> f32;
}

/// Blends scores and sorts `items` best first.
///
/// On entry `item.score` holds the raw match score. It is normalized by the
/// best score in the list (so 0..=1), then every hook's boost is added.
/// Ties: shorter title first, then case-insensitive alphabetical.
pub fn rank(items: &mut [Item], query: &str, hooks: &[&dyn RankHook]) {
    let max = items.iter().map(|i| i.score).fold(0.0_f32, f32::max);
    for it in items.iter_mut() {
        let base = if max > 0.0 { it.score / max } else { 0.0 };
        let boost: f32 = hooks.iter().map(|h| h.boost(query, &it.id.0)).sum();
        it.score = base + boost;
    }
    items.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.title.chars().count().cmp(&b.title.chars().count()))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{FuzzyMatcher, MatchIndex};
    use crate::types::{ItemId, ItemKind};

    fn item(id: &str, title: &str, score: f32) -> Item {
        Item {
            id: ItemId(id.into()),
            kind: ItemKind::App,
            title: title.into(),
            subtitle: String::new(),
            icon: None,
            score,
            source: "apps".into(),
            actions: vec![],
            payload: serde_json::Value::Null,
        }
    }

    struct Boost(&'static str, f32);
    impl RankHook for Boost {
        fn boost(&self, _q: &str, id: &str) -> f32 {
            if id == self.0 { self.1 } else { 0.0 }
        }
    }

    #[test]
    fn nucleo_prefers_prefix_over_later_match() {
        // Documented behaviour: with prefer_prefix, a name starting with the
        // query ("Codec Tool") outscores one where it appears after a word
        // boundary ("Visual Studio Code").
        let names = ["Visual Studio Code", "Codec Tool"];
        let idx = MatchIndex::new(&names);
        let hits = FuzzyMatcher::new().top_k("code", &idx, 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(names[hits[0].idx], "Codec Tool");
    }

    #[test]
    fn boost_reorders() {
        let mut v = vec![item("a", "Alpha", 100.0), item("b", "Beta", 90.0)];
        let b = Boost("b", 0.5);
        rank(&mut v, "x", &[&b]);
        assert_eq!(v[0].id.0, "b");
    }

    #[test]
    fn ties_are_deterministic() {
        let mut v = vec![
            item("1", "beta", 10.0),
            item("2", "Alpha", 10.0),
            item("3", "Zed", 10.0),
            item("4", "alpha", 10.0),
        ];
        rank(&mut v, "", &[]);
        let titles: Vec<_> = v.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["Zed", "beta", "Alpha", "alpha"]);
    }
}
