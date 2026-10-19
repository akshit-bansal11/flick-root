//! Stage 1 of ADR-005: offline natural-language parser (FR-F06-01, FR-F06-02).

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Words that mark an input as natural language.
const MARKERS: &[&str] = &["my", "from", "in", "the", "that", "last", "yesterday"];

/// Words dropped from the name terms.
const FILLER: &[&str] = &[
    "open",
    "find",
    "show",
    "get",
    "me",
    "my",
    "the",
    "a",
    "an",
    "that",
    "which",
    "file",
    "files",
    "called",
    "named",
    "from",
    "in",
    "on",
    "at",
    "of",
    "with",
    "to",
    "into",
    "for",
    "i",
    "it",
    "is",
    "was",
    "were",
    "please",
    "and",
    "about",
    "this",
    "last",
    "downloaded",
    "saved",
    "created",
    "edited",
    "modified",
    "folder",
    "inside",
    "your",
    "our",
    "some",
    "all",
];

/// Words that introduce a location.
const PREPS: &[&str] = &["from", "in", "on", "at", "of", "to", "into", "inside"];

/// Extensions that are never opened from a natural-language request.
const EXECUTABLE: &[&str] = &[
    "exe", "bat", "cmd", "ps1", "lnk", "msi", "vbs", "vbe", "com", "scr", "js", "jse", "wsf",
    "wsh", "reg", "jar", "hta", "cpl", "msc", "dll",
];

const IMG: &[&str] = &["jpg", "jpeg", "png", "webp", "heic", "gif", "bmp"];
const SHEET: &[&str] = &["xlsx", "xls", "csv"];
const DOC: &[&str] = &["docx", "doc", "odt", "rtf", "txt"];
const SLIDE: &[&str] = &["pptx", "ppt", "odp"];
const VIDEO: &[&str] = &["mp4", "mkv", "mov", "avi", "webm"];
const AUDIO: &[&str] = &["mp3", "flac", "wav", "m4a", "ogg"];
const ARCHIVE: &[&str] = &["zip", "7z", "rar", "tar", "gz"];
const PDF: &[&str] = &["pdf"];

/// Type words and the extensions they imply.
const TYPES: &[(&[&str], &[&str])] = &[
    (
        &[
            "photo",
            "photos",
            "picture",
            "pictures",
            "image",
            "images",
            "pic",
            "pics",
            "pfp",
            "screenshot",
            "screenshots",
        ],
        IMG,
    ),
    (&["pdf", "pdfs"], PDF),
    (
        &["sheet", "sheets", "spreadsheet", "spreadsheets", "excel"],
        SHEET,
    ),
    (&["doc", "docs", "document", "documents", "word"], DOC),
    (
        &[
            "slide",
            "slides",
            "deck",
            "decks",
            "presentation",
            "presentations",
            "ppt",
            "powerpoint",
        ],
        SLIDE,
    ),
    (
        &["video", "videos", "clip", "clips", "movie", "movies"],
        VIDEO,
    ),
    (
        &["song", "songs", "music", "audio", "track", "tracks"],
        AUDIO,
    ),
    (&["zip", "archive", "archives"], ARCHIVE),
];

/// Windows known folders a phrase can point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownRoot {
    /// The Desktop folder.
    Desktop,
    /// The Documents folder.
    Documents,
    /// The Downloads folder.
    Downloads,
    /// The Pictures folder.
    Pictures,
    /// The Music folder.
    Music,
    /// The Videos folder.
    Videos,
}

/// Inclusive date range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    /// First day.
    pub start: NaiveDate,
    /// Last day.
    pub end: NaiveDate,
}

/// Result of parsing a natural-language phrase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// Remaining name words; may be empty (match by filters only).
    pub terms: Vec<String>,
    /// Location roots, in phrase order.
    pub roots: Vec<KnownRoot>,
    /// Allowed extensions (lowercase, no dot); empty means any type.
    pub exts: Vec<String>,
    /// Modified-date filter.
    pub modified: Option<DateRange>,
}

fn tokens(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
                .trim_end_matches('.')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// True when the input has 3+ words or contains a marker word (ADR-005).
pub fn is_natural(input: &str) -> bool {
    let t = tokens(input);
    t.len() >= 3 || t.iter().any(|w| MARKERS.contains(&w.as_str()))
}

/// True for extensions (with or without leading dot, any case) that must never auto-open.
pub fn is_executable(ext: &str) -> bool {
    let e = ext.trim_start_matches('.').to_lowercase();
    EXECUTABLE.contains(&e.as_str())
}

fn root_alias(w: &str) -> Option<KnownRoot> {
    Some(match w {
        "desktop" => KnownRoot::Desktop,
        "documents" | "document" | "docs" => KnownRoot::Documents,
        "downloads" | "download" => KnownRoot::Downloads,
        "pictures" | "picture" | "photos" | "images" => KnownRoot::Pictures,
        "music" => KnownRoot::Music,
        "videos" | "video" => KnownRoot::Videos,
        _ => return None,
    })
}

fn weekday(w: &str) -> Option<Weekday> {
    Some(match w {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

fn month(w: &str) -> Option<u32> {
    const M: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    M.iter().position(|m| *m == w).map(|i| i as u32 + 1)
}

fn day(d: NaiveDate) -> DateRange {
    DateRange { start: d, end: d }
}

fn month_range(y: i32, m: u32) -> DateRange {
    let start = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(NaiveDate::MIN);
    let next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    };
    let end = next.and_then(|n| n.pred_opt()).unwrap_or(NaiveDate::MAX);
    DateRange { start, end }
}

fn year_range(y: i32) -> DateRange {
    DateRange {
        start: NaiveDate::from_ymd_opt(y, 1, 1).unwrap_or(NaiveDate::MIN),
        end: NaiveDate::from_ymd_opt(y, 12, 31).unwrap_or(NaiveDate::MAX),
    }
}

/// Monday-to-Sunday week, `weeks_back` weeks before the current one.
fn week_range(today: NaiveDate, weeks_back: i64) -> DateRange {
    let into_week = i64::from(today.weekday().num_days_from_monday());
    let start = today - Duration::days(into_week + 7 * weeks_back);
    DateRange {
        start,
        end: start + Duration::days(6),
    }
}

/// Most recent strictly-past occurrence of `wd`.
fn past_weekday(today: NaiveDate, wd: Weekday) -> NaiveDate {
    let diff = (today.weekday().num_days_from_monday() + 7 - wd.num_days_from_monday()) % 7;
    today - Duration::days(if diff == 0 { 7 } else { i64::from(diff) })
}

/// Most recent occurrence of month `m` that is not in the future.
fn past_month(today: NaiveDate, m: u32) -> DateRange {
    let y = if m <= today.month() {
        today.year()
    } else {
        today.year() - 1
    };
    month_range(y, m)
}

/// Parses a time phrase starting at `t[i]`; returns the range and tokens consumed.
fn time_phrase(t: &[String], i: usize, today: NaiveDate) -> Option<(DateRange, usize)> {
    let w = t[i].as_str();
    let next = t.get(i + 1).map(String::as_str);
    let third = t.get(i + 2).map(String::as_str);
    let num = |s: &str| s.parse::<i64>().ok();
    match (w, next) {
        ("today", _) => Some((day(today), 1)),
        ("yesterday", _) => Some((day(today - Duration::days(1)), 1)),
        ("this", Some("week")) => Some((week_range(today, 0), 2)),
        ("last", Some("week")) => Some((week_range(today, 1), 2)),
        ("this", Some("month")) => Some((month_range(today.year(), today.month()), 2)),
        ("last", Some("month")) => {
            let (y, m) = if today.month() == 1 {
                (today.year() - 1, 12)
            } else {
                (today.year(), today.month() - 1)
            };
            Some((month_range(y, m), 2))
        }
        ("this", Some("year")) => Some((year_range(today.year()), 2)),
        ("last", Some("year")) => Some((year_range(today.year() - 1), 2)),
        ("last", Some(n)) if matches!(third, Some("day" | "days")) => {
            let d = num(n)?;
            Some((
                DateRange {
                    start: today - Duration::days(d),
                    end: today,
                },
                3,
            ))
        }
        (n, Some("day" | "days")) if third == Some("ago") => {
            Some((day(today - Duration::days(num(n)?)), 3))
        }
        _ => match weekday(w) {
            Some(wd) => Some((day(past_weekday(today, wd)), 1)),
            None => month(w).map(|m| (past_month(today, m), 1)),
        },
    }
}

fn add_exts(out: &mut Vec<String>, exts: &[&str]) {
    for e in exts {
        if !out.iter().any(|o| o == e) {
            out.push((*e).to_string());
        }
    }
}

fn is_ext(e: &str) -> bool {
    (1..=5).contains(&e.len())
        && e.chars().all(|c| c.is_ascii_alphanumeric())
        && e.chars().any(|c| c.is_ascii_alphabetic())
}

/// Parses a natural-language phrase into name terms and filters. `today` is injected for tests.
pub fn parse(input: &str, today: NaiveDate) -> Parsed {
    let t = tokens(input);
    let mut p = Parsed::default();
    let mut i = 0;
    while i < t.len() {
        let w = t[i].as_str();
        // Location: "<prep> [the|my] <root>" or "<root> folder".
        if PREPS.contains(&w) {
            let mut j = i + 1;
            while matches!(t.get(j).map(String::as_str), Some("the" | "my" | "your")) {
                j += 1;
            }
            if let Some(r) = t.get(j).and_then(|x| root_alias(x)) {
                p.roots.push(r);
                i = j + 1;
                continue;
            }
        }
        if t.get(i + 1).map(String::as_str) == Some("folder")
            && let Some(r) = root_alias(w)
        {
            p.roots.push(r);
            i += 2;
            continue;
        }
        if let Some((range, used)) = time_phrase(&t, i, today) {
            p.modified = Some(range);
            i += used;
            continue;
        }
        if let Some((_, exts)) = TYPES.iter().find(|(words, _)| words.contains(&w)) {
            add_exts(&mut p.exts, exts);
        } else if let Some(ext) = w.strip_prefix('.').filter(|e| is_ext(e)) {
            add_exts(&mut p.exts, &[ext]);
        } else if let Some((name, ext)) = w
            .rsplit_once('.')
            .filter(|(n, e)| !n.is_empty() && is_ext(e))
        {
            p.terms.push(name.to_string());
            add_exts(&mut p.exts, &[ext]);
        } else if !FILLER.contains(&w) {
            p.terms.push(w.to_string());
        }
        i += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use KnownRoot::*;

    fn d(y: i32, m: u32, dd: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, dd).unwrap()
    }
    fn r(a: (i32, u32, u32), b: (i32, u32, u32)) -> Option<DateRange> {
        Some(DateRange {
            start: d(a.0, a.1, a.2),
            end: d(b.0, b.1, b.2),
        })
    }
    fn one(a: (i32, u32, u32)) -> Option<DateRange> {
        r(a, a)
    }

    type Case = (
        &'static str,
        &'static [&'static str],
        &'static [KnownRoot],
        &'static [&'static str],
        Option<DateRange>,
    );

    #[test]
    fn phrases() {
        // today = Wednesday 2024-05-15
        let today = d(2024, 5, 15);
        let none: &[&str] = &[];
        let cases: Vec<Case> = vec![
            ("open my pfp from pictures", none, &[Pictures], IMG, None),
            ("pdf from yesterday", none, &[], PDF, one((2024, 5, 14))),
            (
                "the invoice i downloaded last week",
                &["invoice"],
                &[],
                none,
                r((2024, 5, 6), (2024, 5, 12)),
            ),
            ("screenshots from today", none, &[], IMG, one((2024, 5, 15))),
            ("budget sheet", &["budget"], &[], SHEET, None),
            ("resume.pdf", &["resume"], &[], PDF, None),
            (
                "photos from march",
                none,
                &[],
                IMG,
                r((2024, 3, 1), (2024, 3, 31)),
            ),
            ("song from downloads", none, &[Downloads], AUDIO, None),
            (
                "that deck from last month",
                none,
                &[],
                SLIDE,
                r((2024, 4, 1), (2024, 4, 30)),
            ),
            ("notes on desktop", &["notes"], &[Desktop], none, None),
            (
                "video from 3 days ago",
                none,
                &[],
                VIDEO,
                one((2024, 5, 12)),
            ),
            ("files from monday", none, &[], none, one((2024, 5, 13))),
            (
                "find tax return in my documents",
                &["tax", "return"],
                &[Documents],
                none,
                None,
            ),
            (
                "show me the report on the desktop",
                &["report"],
                &[Desktop],
                none,
                None,
            ),
            (
                "get my music from the music folder",
                none,
                &[Music],
                AUDIO,
                None,
            ),
            ("downloads folder zip", none, &[Downloads], ARCHIVE, None),
            (
                "archive from this week",
                none,
                &[],
                ARCHIVE,
                r((2024, 5, 13), (2024, 5, 19)),
            ),
            (
                "slides from this month",
                none,
                &[],
                SLIDE,
                r((2024, 5, 1), (2024, 5, 31)),
            ),
            (
                "spreadsheet from last year",
                none,
                &[],
                SHEET,
                r((2023, 1, 1), (2023, 12, 31)),
            ),
            (
                "excel this year",
                none,
                &[],
                SHEET,
                r((2024, 1, 1), (2024, 12, 31)),
            ),
            (
                "document from last 7 days",
                none,
                &[],
                DOC,
                r((2024, 5, 8), (2024, 5, 15)),
            ),
            ("movie called inception", &["inception"], &[], VIDEO, None),
            ("image named logo", &["logo"], &[], IMG, None),
            ("clip from friday", none, &[], VIDEO, one((2024, 5, 10))),
            (
                "notes from wednesday",
                &["notes"],
                &[],
                none,
                one((2024, 5, 8)),
            ),
            (
                "report in june",
                &["report"],
                &[],
                none,
                r((2023, 6, 1), (2023, 6, 30)),
            ),
            (
                "report in may",
                &["report"],
                &[],
                none,
                r((2024, 5, 1), (2024, 5, 31)),
            ),
            (
                "report in december",
                &["report"],
                &[],
                none,
                r((2023, 12, 1), (2023, 12, 31)),
            ),
            ("a .pdf", none, &[], PDF, None),
            (".docx contract", &["contract"], &[], &["docx"], None),
            ("Notes.TXT", &["notes"], &[], &["txt"], None),
            ("PHOTOS FROM PICTURES!", none, &[Pictures], IMG, None),
            ("budget, sheet.", &["budget"], &[], SHEET, None),
            (
                "pictures folder holiday",
                &["holiday"],
                &[Pictures],
                none,
                None,
            ),
            (
                "holiday photos from the pictures folder",
                &["holiday"],
                &[Pictures],
                IMG,
                None,
            ),
            (
                "meeting notes from last month on desktop",
                &["meeting", "notes"],
                &[Desktop],
                none,
                r((2024, 4, 1), (2024, 4, 30)),
            ),
            (
                "find the pdf called taxes from last year",
                &["taxes"],
                &[],
                PDF,
                r((2023, 1, 1), (2023, 12, 31)),
            ),
            (
                "pic pdf",
                none,
                &[],
                &["jpg", "jpeg", "png", "webp", "heic", "gif", "bmp", "pdf"],
                None,
            ),
            ("photo photos", none, &[], IMG, None),
            (
                "which file was modified yesterday",
                none,
                &[],
                none,
                one((2024, 5, 14)),
            ),
            ("hello", &["hello"], &[], none, None),
            ("", none, &[], none, None),
            ("open", none, &[], none, None),
            ("last", none, &[], none, None),
            ("in videos", none, &[Videos], none, None),
            ("ppt from the docs", none, &[Documents], SLIDE, None),
            (
                "song track 2 days ago",
                none,
                &[],
                AUDIO,
                one((2024, 5, 13)),
            ),
            (
                "report today yesterday",
                &["report"],
                &[],
                none,
                one((2024, 5, 14)),
            ),
        ];
        assert!(cases.len() >= 40);
        for (input, terms, roots, exts, modified) in cases {
            let p = parse(input, today);
            assert_eq!(p.terms, terms, "terms: {input}");
            assert_eq!(p.roots, roots, "roots: {input}");
            assert_eq!(p.exts, exts, "exts: {input}");
            assert_eq!(p.modified, modified, "modified: {input}");
        }
    }

    #[test]
    fn weekday_same_day_is_a_week_ago_and_january_rolls_back() {
        let mon = d(2024, 5, 13);
        assert_eq!(parse("monday", mon).modified, one((2024, 5, 6)));
        assert_eq!(
            parse("last month", d(2024, 1, 10)).modified,
            r((2023, 12, 1), (2023, 12, 31))
        );
    }

    #[test]
    fn natural_detection() {
        for s in [
            "open my file",
            "a b c",
            "from x",
            "the x",
            "last",
            "yesterday",
            "invoice.pdf in",
            "My Photo",
        ] {
            assert!(is_natural(s), "{s}");
        }
        for s in ["", "budget", "budget sheet", "notes pdf", "Chrome"] {
            assert!(!is_natural(s), "{s}");
        }
    }

    #[test]
    fn executables() {
        for e in [
            "exe", ".EXE", "bat", "cmd", "ps1", "lnk", "msi", "vbs", "com", "scr", "js",
        ] {
            assert!(is_executable(e), "{e}");
        }
        for e in ["pdf", ".png", "txt", ""] {
            assert!(!is_executable(e), "{e}");
        }
    }
}
