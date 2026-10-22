//! User exclusion list (`[apps] skip`): folders the programs scan never enters.
//!
//! Entries are only compared to paths, never executed, passed to a shell or opened. Three forms,
//! all case-insensitive and matched on folder boundaries (`D:\a` never excludes `D:\ab`):
//! a drive (`E:\`), a path under a drive (`D:\games`), and a drive-less pattern whose segments
//! match any run of consecutive folder names (`node_modules`, `*cache*`, `target\*\deps`), where
//! `*` stands for part of one name. Matching is linear in the length of the path and pattern:
//! no backtracking, so a pattern like `*a*a*a*b` cannot blow up.

use std::path::Path;

/// Compiled exclusion list; build it once per scan from the validated config entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkipList {
    /// Excluded drive letters, lowercase.
    drives: Vec<char>,
    /// Excluded folders as lowercase `X:\a\b` (no trailing separator).
    prefixes: Vec<String>,
    /// Drive-less patterns, lowercase, one entry per segment.
    patterns: Vec<Vec<String>>,
    /// Segment count of the longest pattern.
    longest: usize,
}

impl SkipList {
    /// Compile `entries`. Anything unusable (empty, empty segment, only `*`) is ignored, so an
    /// unvalidated list can neither crash nor exclude everything.
    pub fn new(entries: &[String]) -> Self {
        let mut s = Self::default();
        for raw in entries {
            let lower = raw.trim().replace('/', "\\").to_lowercase();
            let e = lower.trim_end_matches('\\');
            let b = e.as_bytes();
            let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
            if drive && b.len() == 2 {
                s.drives.push(b[0] as char);
            } else if drive && b[2] == b'\\' {
                s.prefixes.push(e.to_string());
            } else if !drive && !e.is_empty() {
                let segs: Vec<String> = e.split('\\').map(String::from).collect();
                let junk = segs.iter().any(String::is_empty)
                    || segs.iter().all(|g| g.bytes().all(|c| c == b'*'));
                if !junk {
                    s.longest = s.longest.max(segs.len());
                    s.patterns.push(segs);
                }
            }
        }
        s
    }

    /// Whether the directory at `path` is excluded: its drive, a folder it lies in or is, or a
    /// pattern matching its last folder names (ancestors were already tested during the walk).
    pub fn excludes_dir(&self, path: &Path) -> bool {
        if self.drives.is_empty() && self.prefixes.is_empty() && self.patterns.is_empty() {
            return false;
        }
        let lower = path.to_string_lossy().to_lowercase();
        let p = lower.trim_end_matches('\\');
        let b = p.as_bytes();
        if b.len() >= 2 && b[1] == b':' && self.drives.contains(&(b[0] as char)) {
            return true;
        }
        let under = |x: &String| {
            p.strip_prefix(x.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('\\'))
        };
        if self.prefixes.iter().any(under) {
            return true;
        }
        if self.patterns.is_empty() {
            return false;
        }
        // Last folder first.
        let tail: Vec<&str> = p.rsplit('\\').take(self.longest).collect();
        self.patterns.iter().any(|pat| {
            pat.len() <= tail.len()
                && pat
                    .iter()
                    .rev()
                    .zip(&tail)
                    .all(|(seg, name)| glob(seg, name))
        })
    }
}

/// Whether `s` matches `pat`, where `*` matches any run of characters (both already lowercase).
/// Linear: the pieces between stars are located left to right with `str::find` (two-way search).
fn glob(pat: &str, s: &str) -> bool {
    let mut pieces = pat.split('*');
    let Some(mut rest) = s.strip_prefix(pieces.next().unwrap_or("")) else {
        return false;
    };
    let mut pieces: Vec<&str> = pieces.collect();
    let Some(last) = pieces.pop() else {
        return rest.is_empty(); // no star: the whole name must equal the pattern
    };
    for piece in pieces {
        let Some(i) = rest.find(piece) else {
            return false;
        };
        rest = &rest[i + piece.len()..];
    }
    rest.ends_with(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skip(entries: &[&str]) -> SkipList {
        SkipList::new(&entries.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    fn hit(list: &SkipList, path: &str) -> bool {
        list.excludes_dir(Path::new(path))
    }

    #[test]
    fn drive_prefix_and_boundaries() {
        let l = skip(&["e:", r"D:\Games", "d:/tools/"]);
        assert!(hit(&l, r"E:\"));
        assert!(hit(&l, r"e:\anything\deep"));
        assert!(!hit(&l, r"F:\x"));
        assert!(hit(&l, r"d:\GAMES"));
        assert!(hit(&l, r"D:\games\halo"));
        assert!(!hit(&l, r"D:\gamesx"));
        assert!(!hit(&l, r"D:\games2\x"));
        assert!(hit(&l, r"D:\Tools\bin"));
        assert!(!hit(&l, r"C:\games"));
    }

    #[test]
    fn name_globs_and_multi_segment_patterns() {
        let l = skip(&[
            "node_modules",
            ".*",
            "*cache*",
            r"target\*\deps",
            r"target\*\build",
        ]);
        assert!(hit(&l, r"C:\p\node_modules"));
        assert!(hit(&l, r"C:\p\NODE_MODULES"));
        assert!(!hit(&l, r"C:\p\node_modules2"));
        assert!(hit(&l, r"C:\Users\me\.claude"));
        assert!(!hit(&l, r"C:\Users\me\claude"));
        assert!(hit(&l, r"C:\x\MyCache2"));
        assert!(hit(&l, r"C:\x\cache"));
        assert!(!hit(&l, r"C:\x\cash"));
        assert!(hit(&l, r"C:\w\target\debug\deps"));
        assert!(hit(&l, r"C:\w\target\release\build"));
        assert!(!hit(&l, r"C:\w\target\release"));
        assert!(!hit(&l, r"C:\w\target\debug\deps\more"));
        assert!(!hit(&l, r"C:\w\deps"));
        assert!(!hit(&l, r"C:\w\other\debug\deps"));
    }

    #[test]
    fn empty_and_unusable_entries_exclude_nothing() {
        let l = skip(&["", "  ", "*", "**", r"a\\b", r"\\srv\share"]);
        assert_eq!(l, SkipList::default());
        assert!(!hit(&l, r"C:\anything"));
        assert!(!hit(&SkipList::default(), r"C:\anything"));
    }

    #[test]
    fn adversarial_pattern_is_fast() {
        let pat = format!("{}b", "*a".repeat(120));
        let l = skip(&[pat.as_str()]);
        let name = "a".repeat(250);
        let t = std::time::Instant::now();
        for _ in 0..1000 {
            assert!(!l.excludes_dir(Path::new(&format!(r"C:\{name}"))));
        }
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
        assert!(glob("*a*a*a*b", "xaxaxab"));
        assert!(!glob("*a*a*a*b", "aaaa"));
        assert!(glob("a*b*c", "abxbc"));
        assert!(!glob("a*b*c", "acb"));
        assert!(glob("ab*", "ab"));
        assert!(!glob("*ab*b", "ab"));
    }
}
