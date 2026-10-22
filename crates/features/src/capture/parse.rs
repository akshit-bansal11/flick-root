//! Local parser for capture input: `log 250 lunch #food yesterday 12:30`, `note ...`, `todo ...`.
//! Pure; no I/O. Amount, tags, date and time are found offline; the rest is the text.

use crate::search::fuzzy_pick;
use chrono::{Duration, NaiveDate, NaiveTime};
use serde_json::{Value, json};

/// Which keyword started the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `log ...`: an entry, usually with an amount.
    Log,
    /// `note ...`: raw text.
    Note,
    /// `todo ...`: a task.
    Todo,
}

/// One captured entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// Keyword used.
    pub kind: Kind,
    /// The text without the recognised fields (raw text for notes and todos).
    pub text: String,
    /// First amount found (`250`, `$12.50`, `1.5k`).
    pub amount: Option<f64>,
    /// `#tags`, lowercase, without the `#`.
    pub tags: Vec<String>,
    /// Date word or ISO date (`today`, `yesterday`, `tomorrow`, `2026-10-19`).
    pub date: Option<NaiveDate>,
    /// `HH:MM` (24 hour).
    pub time: Option<NaiveTime>,
}

/// Splits the leading keyword from the rest. `None` when the text is not a capture.
pub fn split_keyword(text: &str) -> Option<(Kind, &str)> {
    let t = text.trim_start();
    let (head, rest) = t.split_once(char::is_whitespace)?;
    let head = head.to_lowercase();
    // "log" is too short to fuzz ("logs" would collide), so only note and todo accept a typo.
    let kind = match head.as_str() {
        "log" => Kind::Log,
        "note" => Kind::Note,
        "todo" => Kind::Todo,
        h => match fuzzy_pick(h, ["note", "todo"]) {
            Some("note") => Kind::Note,
            Some("todo") => Kind::Todo,
            _ => return None,
        },
    };
    Some((kind, rest.trim()))
}

fn amount(tok: &str) -> Option<f64> {
    let t = tok.trim_start_matches(['\u{20b9}', '$', '\u{20ac}', '\u{a3}']);
    let (num, mult) = match t.strip_suffix(['k', 'K']) {
        Some(n) => (n, 1000.0),
        None => (t, 1.0),
    };
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    num.parse::<f64>().ok().map(|n| n * mult)
}

fn date(tok: &str, today: NaiveDate) -> Option<NaiveDate> {
    match tok.to_lowercase().as_str() {
        "today" => Some(today),
        "yesterday" => Some(today - Duration::days(1)),
        "tomorrow" => Some(today + Duration::days(1)),
        _ => NaiveDate::parse_from_str(tok, "%Y-%m-%d").ok(),
    }
}

fn time(tok: &str) -> Option<NaiveTime> {
    tok.contains(':')
        .then(|| NaiveTime::parse_from_str(tok, "%H:%M").ok())
        .flatten()
}

fn tag(tok: &str) -> Option<String> {
    let t = tok.strip_prefix('#')?;
    (!t.is_empty()
        && t.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-'))
    .then(|| t.to_lowercase())
}

/// Parses the text after the keyword. `today` is passed in so tests are stable.
pub fn parse(kind: Kind, rest: &str, today: NaiveDate) -> Parsed {
    let mut p = Parsed {
        kind,
        text: String::new(),
        amount: None,
        tags: Vec::new(),
        date: None,
        time: None,
    };
    if kind != Kind::Log {
        p.text = rest.trim().to_owned();
        p.tags = rest.split_whitespace().filter_map(tag).collect();
        return p;
    }
    let mut words = Vec::new();
    for tok in rest.split_whitespace() {
        if let Some(t) = tag(tok) {
            p.tags.push(t);
        } else if p.amount.is_none()
            && let Some(a) = amount(tok)
        {
            p.amount = Some(a);
        } else if p.date.is_none()
            && let Some(d) = date(tok, today)
        {
            p.date = Some(d);
        } else if p.time.is_none()
            && let Some(t) = time(tok)
        {
            p.time = Some(t);
        } else {
            words.push(tok);
        }
    }
    p.text = words.join(" ");
    p
}

fn fmt_amount(a: f64) -> String {
    if a.fract() == 0.0 {
        format!("{a:.0}")
    } else {
        format!("{a:.2}")
    }
}

impl Parsed {
    /// True when a log line has both an amount and a label; anything else may need the Query provider.
    pub fn is_clear(&self) -> bool {
        self.kind != Kind::Log || (self.amount.is_some() && !self.text.is_empty())
    }

    /// Short preview for the row subtitle.
    pub fn preview(&self) -> String {
        let mut parts = Vec::new();
        if let Some(a) = self.amount {
            parts.push(fmt_amount(a));
        }
        if !self.text.is_empty() {
            parts.push(self.text.clone());
        }
        parts.extend(self.tags.iter().map(|t| format!("#{t}")));
        if let Some(d) = self.date {
            parts.push(d.to_string());
        }
        parts.join(" \u{b7} ")
    }

    /// The `fields` object sent to a webhook.
    pub fn fields(&self) -> Value {
        let kind = match self.kind {
            Kind::Log => "log",
            Kind::Note => "note",
            Kind::Todo => "todo",
        };
        json!({
            "kind": kind,
            "amount": self.amount,
            "tags": self.tags,
            "date": self.date.map(|d| d.to_string()),
            "time": self.time.map(|t| t.format("%H:%M").to_string()),
        })
    }

    /// The Markdown line to append, given the current time for entries without one.
    pub fn line(&self, now: NaiveTime) -> String {
        let text = self.text.replace(['\r', '\n'], " ");
        let mut s = if self.kind == Kind::Todo {
            format!("- [ ] {text}")
        } else {
            let t = self.time.unwrap_or(now);
            format!("- {} {text}", t.format("%H:%M"))
        };
        if let Some(a) = self.amount {
            s.push_str(&format!(" ({})", fmt_amount(a)));
        }
        if self.kind == Kind::Log {
            for t in &self.tags {
                s.push_str(&format!(" #{t}"));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
    }

    fn log(s: &str) -> Parsed {
        let text = format!("log {s}");
        let (k, rest) = split_keyword(&text).unwrap();
        parse(k, rest, today())
    }

    #[test]
    fn keywords() {
        assert_eq!(split_keyword("log 250 lunch").unwrap().0, Kind::Log);
        assert_eq!(split_keyword("NOTE hi").unwrap(), (Kind::Note, "hi"));
        assert_eq!(split_keyword("todo  buy milk").unwrap().1, "buy milk");
        assert!(split_keyword("log").is_none());
        assert!(split_keyword("logs 1 x").is_none());
        assert_eq!(split_keyword("tood buy milk").unwrap().0, Kind::Todo);
        assert_eq!(split_keyword("noet hi").unwrap().0, Kind::Note);
        assert!(split_keyword("hello world").is_none());
    }

    #[test]
    fn log_table() {
        let p = log("250 lunch");
        assert_eq!((p.amount, p.text.as_str()), (Some(250.0), "lunch"));
        assert!(p.is_clear());
        let p = log("lunch with ana 12.5 #Food #team yesterday 12:30");
        assert_eq!(p.amount, Some(12.5));
        assert_eq!(p.text, "lunch with ana");
        assert_eq!(p.tags, ["food", "team"]);
        assert_eq!(p.date, NaiveDate::from_ymd_opt(2026, 10, 18));
        assert_eq!(p.time, NaiveTime::from_hms_opt(12, 30, 0));
        assert_eq!(log("$9.99 coffee").amount, Some(9.99));
        assert_eq!(log("1.5k rent").amount, Some(1500.0));
        assert_eq!(
            log("taxi 2026-01-02").date,
            NaiveDate::from_ymd_opt(2026, 1, 2)
        );
        assert!(!log("lunch").is_clear());
        assert!(!log("250").is_clear());
    }

    #[test]
    fn note_and_todo_stay_raw() {
        let (k, r) = split_keyword("note call 250 people #idea").unwrap();
        let p = parse(k, r, today());
        assert_eq!(p.text, "call 250 people #idea");
        assert_eq!(p.amount, None);
        assert_eq!(p.tags, ["idea"]);
        assert!(p.is_clear());
    }

    #[test]
    fn lines() {
        let now = NaiveTime::from_hms_opt(9, 5, 0).unwrap();
        assert_eq!(
            log("250 lunch #food").line(now),
            "- 09:05 lunch (250) #food"
        );
        let (k, r) = split_keyword("todo buy milk").unwrap();
        assert_eq!(parse(k, r, today()).line(now), "- [ ] buy milk");
    }
}
