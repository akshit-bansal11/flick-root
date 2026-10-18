//! Mode prefix parsing and mode cycling.

use crate::types::Mode;

/// Which modes are enabled. Command is always enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSet {
    /// Find mode enabled.
    pub find: bool,
    /// Query mode enabled.
    pub query: bool,
}

impl ModeSet {
    fn has(&self, m: Mode) -> bool {
        match m {
            Mode::Command => true,
            Mode::Find => self.find,
            Mode::Query => self.query,
        }
    }
}

/// Split a leading mode prefix (`>`, `/`, `?`) off `raw`. Only the very first
/// character counts; otherwise the current mode and the whole input are returned.
pub fn parse_prefix(raw: &str, current: Mode) -> (Mode, &str) {
    let mode = match raw.chars().next() {
        Some('>') => Mode::Command,
        Some('/') => Mode::Find,
        Some('?') => Mode::Query,
        _ => return (current, raw),
    };
    (mode, &raw[1..])
}

const CYCLE: [Mode; 3] = [Mode::Command, Mode::Find, Mode::Query];

fn step(current: Mode, enabled: &ModeSet, delta: usize) -> Mode {
    let start = CYCLE.iter().position(|m| *m == current).unwrap_or(0);
    (1..=CYCLE.len())
        .map(|i| CYCLE[(start + i * delta) % CYCLE.len()])
        .find(|m| enabled.has(*m))
        .unwrap_or(Mode::Command)
}

/// Next enabled mode in Command, Find, Query order, wrapping.
pub fn next_mode(current: Mode, enabled: &ModeSet) -> Mode {
    step(current, enabled, 1)
}

/// Previous enabled mode, wrapping.
pub fn prev_mode(current: Mode, enabled: &ModeSet) -> Mode {
    step(current, enabled, CYCLE.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mode::*;

    #[test]
    fn prefix_table() {
        let cases = [
            ("", Find, Find, ""),
            (">", Find, Command, ""),
            ("/docs", Command, Find, "docs"),
            ("?why", Command, Query, "why"),
            (" /docs", Command, Command, " /docs"),
            ("a/b", Query, Query, "a/b"),
            ("é/x", Command, Command, "é/x"),
            ("//", Command, Find, "/"),
        ];
        for (raw, cur, mode, text) in cases {
            assert_eq!(parse_prefix(raw, cur), (mode, text), "{raw:?}");
        }
    }

    #[test]
    fn cycling() {
        let all = ModeSet {
            find: true,
            query: true,
        };
        assert_eq!(next_mode(Command, &all), Find);
        assert_eq!(next_mode(Query, &all), Command);
        assert_eq!(prev_mode(Command, &all), Query);
        let no_find = ModeSet {
            find: false,
            query: true,
        };
        assert_eq!(next_mode(Command, &no_find), Query);
        assert_eq!(prev_mode(Query, &no_find), Command);
        let none = ModeSet {
            find: false,
            query: false,
        };
        assert_eq!(next_mode(Command, &none), Command);
        assert_eq!(prev_mode(Find, &none), Command);
    }
}
