//! The set of known themes: two code defaults, bundled seeds, and user imports.

use std::collections::HashSet;

use super::{Resolved, Seed, Source, ThemeError, resolve};
use crate::brand::{APP_DISPLAY_NAME, DEFAULT_THEME_ID, DEFAULT_THEME_LIGHT_ID};

const FLICK_SEEDS: &str = include_str!("seeds/flick.json");
const PORT_SEEDS: &str = include_str!("seeds/ports.json");

/// A seed together with its derived, contrast-checked roles.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// The theme as authored.
    pub seed: Seed,
    /// Roles derived from the seed.
    pub resolved: Resolved,
}

/// All themes available to the app, in display order.
#[derive(Debug, Clone, PartialEq)]
pub struct Registry {
    themes: Vec<Theme>,
}

fn default_seed(dark: bool) -> Seed {
    let hex = |s: &str| s.to_owned();
    let (id, suffix, c) = if dark {
        (
            DEFAULT_THEME_ID,
            "",
            [
                "#0c0e12", "#14171d", "#e8ebf0", "#b6bbc3", "#5cc8e8", "#1d2a33", "#262b34",
                "#1b1f27",
            ],
        )
    } else {
        (
            DEFAULT_THEME_LIGHT_ID,
            " Light",
            [
                "#f4f8fb", "#ffffff", "#0f2333", "#41545f", "#0b7699", "#d6e7ee", "#d0dee8",
                "#e7eff5",
            ],
        )
    };
    Seed {
        id: id.to_owned(),
        name: format!("{APP_DISPLAY_NAME} Default{suffix}"),
        source: Source::Builtin,
        bg: hex(c[0]),
        panel: hex(c[1]),
        text: hex(c[2]),
        muted: hex(c[3]),
        accent: hex(c[4]),
        sel: Some(hex(c[5])),
        line: Some(hex(c[6])),
        surface: Some(hex(c[7])),
        upstream: None,
    }
}

fn theme_of(seed: Seed) -> Result<Theme, ThemeError> {
    let resolved = resolve(&seed)?;
    Ok(Theme { seed, resolved })
}

fn is_imported_id(id: &str) -> bool {
    id.strip_prefix("imported:").is_some_and(|slug| {
        (1..=64).contains(&slug.len())
            && slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

impl Registry {
    /// Only the two code defaults. Cannot fail: the seeds are constants.
    pub fn defaults_only() -> Self {
        let themes = [true, false]
            .into_iter()
            .filter_map(|dark| theme_of(default_seed(dark)).ok())
            .collect();
        Self { themes }
    }

    /// Code defaults plus every bundled seed. Fails when a bundled seed is invalid or repeats an id.
    pub fn builtin() -> Result<Self, ThemeError> {
        let mut themes = Vec::new();
        let mut seen = HashSet::new();
        let bundled = [FLICK_SEEDS, PORT_SEEDS]
            .into_iter()
            .map(serde_json::from_str::<Vec<Seed>>)
            .collect::<Result<Vec<_>, _>>()?;
        let seeds = [default_seed(true), default_seed(false)]
            .into_iter()
            .chain(bundled.into_iter().flatten());
        for seed in seeds {
            if !seen.insert(seed.id.clone()) {
                return Err(ThemeError::BadId);
            }
            themes.push(theme_of(seed)?);
        }
        Ok(Self { themes })
    }

    /// Adds imported seeds, silently dropping any whose id is not `imported:<slug>` (slug of 1-64
    /// lowercase `a-z`, `0-9` or `-`), that fail to resolve, or that reuse an existing id.
    #[must_use]
    pub fn with_imported(mut self, seeds: Vec<Seed>) -> Self {
        for seed in seeds {
            if !is_imported_id(&seed.id) || self.contains(&seed.id) {
                continue;
            }
            if let Ok(theme) = theme_of(seed) {
                self.themes.push(theme);
            }
        }
        self
    }

    /// The theme with this id.
    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.seed.id == id)
    }

    /// Whether a theme with this id exists.
    pub fn contains(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// Every theme, in display order.
    pub fn list(&self) -> &[Theme] {
        &self.themes
    }

    /// The code default for the dark or light scheme.
    pub fn default_for(&self, dark: bool) -> Option<&Theme> {
        self.get(if dark {
            DEFAULT_THEME_ID
        } else {
            DEFAULT_THEME_LIGHT_ID
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THIRD_PARTY: &str = include_str!("../../../../THIRD-PARTY-THEMES.md");
    const HEADER: &str =
        "| id | name | repo URL | theme file URL | SPDX licence | licence file URL | copyright |";
    const ALLOWED: [&str; 6] = [
        "MIT",
        "ISC",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "0BSD",
        "CC0-1.0",
    ];

    #[test]
    fn third_party_entries_cover_every_port() {
        assert!(THIRD_PARTY.lines().any(|l| l.trim() == HEADER));
        let rows: Vec<Vec<&str>> = THIRD_PARTY
            .lines()
            .skip_while(|l| l.trim() != HEADER)
            .skip(2)
            .take_while(|l| l.starts_with('|'))
            .map(|l| {
                l.trim()
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect()
            })
            .collect();
        let registry = Registry::builtin().unwrap();
        let ports: Vec<&Seed> = registry
            .list()
            .iter()
            .map(|t| &t.seed)
            .filter(|s| s.source == Source::Port)
            .collect();
        for seed in &ports {
            let n = rows.iter().filter(|r| r[0] == seed.id).count();
            assert_eq!(n, 1, "port {} needs exactly one row", seed.id);
        }
        for row in &rows {
            let &[id, _, repo, file, spdx, licence, copyright] = &row[..] else {
                panic!("row needs 7 cells: {row:?}");
            };
            let seed = ports.iter().find(|s| s.id == id);
            let seed = seed.unwrap_or_else(|| panic!("row {id} has no port seed"));
            for url in [repo, file, licence] {
                assert!(url.starts_with("https://"), "{id}: {url} is not https");
            }
            assert!(ALLOWED.contains(&spdx), "{id}: {spdx} is not allowed");
            assert!(!copyright.is_empty(), "{id}: copyright is empty");
            assert_eq!(seed.upstream.as_deref(), Some(file), "{id}: upstream");
        }
    }
}
