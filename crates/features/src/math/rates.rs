//! ECB daily reference rates (F02, FR-F02-03/04): parse, convert, parse currency queries.
//! Everything here is pure; the fetch task lives in `mod.rs`.

use chrono::NaiveDate;
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// ECB daily reference rates, HTTPS only.
pub const URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// Parsing failure.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum RatesError {
    /// The XML is not well formed.
    #[error("malformed rates XML: {0}")]
    Xml(String),
    /// No dated `Cube` or no rates inside it.
    #[error("rates XML has no dated rates")]
    Empty,
}

/// One day of rates: units of each currency per euro.
#[derive(Debug, Clone, PartialEq)]
pub struct Rates {
    /// ECB reference date, `YYYY-MM-DD`.
    pub date: String,
    /// Fetch time, Unix ms.
    pub fetched_at: i64,
    /// Currency code to units per EUR; always contains EUR = 1.
    pub per_eur: HashMap<String, f64>,
}

/// Parses the ECB daily XML.
pub fn parse(xml: &str, fetched_at: i64) -> Result<Rates, RatesError> {
    let mut reader = Reader::from_str(xml);
    let (mut date, mut per_eur) = (None::<String>, HashMap::new());
    loop {
        match reader
            .read_event()
            .map_err(|e| RatesError::Xml(e.to_string()))?
        {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "Cube" => {
                let (mut time, mut cur, mut rate) = (None, None, None);
                for a in e.attributes().flatten() {
                    let v = a
                        .normalized_value(quick_xml::XmlVersion::default())
                        .map_err(|e| RatesError::Xml(e.to_string()))?;
                    match a.key.as_ref() {
                        "time" => time = Some(v.into_owned()),
                        "currency" => cur = Some(v.into_owned()),
                        "rate" => rate = v.parse::<f64>().ok(),
                        _ => {}
                    }
                }
                if let Some(t) = time {
                    date = Some(t);
                }
                if let (Some(c), Some(r)) = (cur, rate)
                    && r > 0.0
                    && r.is_finite()
                {
                    per_eur.insert(c.to_uppercase(), r);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let date = date.ok_or(RatesError::Empty)?;
    if per_eur.is_empty() {
        return Err(RatesError::Empty);
    }
    per_eur.insert("EUR".into(), 1.0);
    Ok(Rates {
        date,
        fetched_at,
        per_eur,
    })
}

impl Rates {
    /// Converts `amount` of `from` into `to`; `None` for an unknown currency.
    pub fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        Some(amount * self.per_eur.get(to)? / self.per_eur.get(from)?)
    }

    /// Whole days between the reference date and `today`, 0 when unparseable or in the future.
    pub fn age_days(&self, today: NaiveDate) -> i64 {
        NaiveDate::parse_from_str(&self.date, "%Y-%m-%d")
            .map_or(0, |d| (today - d).num_days().max(0))
    }

    /// Resolves a symbol, word or ISO code to a code we have a rate for.
    fn code(&self, token: &str) -> Option<String> {
        let upper = token.to_uppercase();
        let mapped = match token.to_lowercase().as_str() {
            "$" | "dollar" | "dollars" => "USD",
            "€" | "euro" | "euros" => "EUR",
            "₹" | "rupee" | "rupees" => "INR",
            "£" | "pound" | "pounds" => "GBP",
            "¥" | "yen" => "JPY",
            _ => upper.as_str(),
        }
        .to_string();
        self.per_eur.contains_key(&mapped).then_some(mapped)
    }
}

const SYM: &str = "[$€₹£¥]";
const NUM: &str = r"(?:[0-9][0-9,]*(?:\.[0-9]+)?|\.[0-9]+)";

static QUERY: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?ix)^(?:(?P<fs>{SYM})\s*)?(?P<n>{NUM})(?:\s*(?P<fw>\p{{L}}+|{SYM}))?\s+(?:in|to|as)\s+(?P<to>\p{{L}}+|{SYM})$"
    ))
    .ok()
});

/// A parsed currency question.
#[derive(Debug, PartialEq)]
pub struct Ask {
    /// Amount typed.
    pub amount: f64,
    /// Source code.
    pub from: String,
    /// Target code.
    pub to: String,
}

/// Parses "250 usd in inr", "$20 in inr", "₹500 to usd" against the known currencies.
pub fn parse_ask(text: &str, rates: &Rates) -> Option<Ask> {
    let caps = QUERY.as_ref()?.captures(text.trim())?;
    let from = caps.name("fs").or_else(|| caps.name("fw"))?.as_str();
    if caps.name("fs").is_some() && caps.name("fw").is_some() {
        return None;
    }
    let amount = caps["n"].replace(',', "").parse().ok()?;
    Some(Ask {
        amount,
        from: rates.code(from)?,
        to: rates.code(&caps["to"])?,
    })
}

/// Result text such as "26,716.00 INR"-free plain form: two decimals, four when under 1.
pub fn format_amount(v: f64, code: &str) -> String {
    if v.abs() >= 1.0 {
        format!("{v:.2} {code}")
    } else {
        format!("{v:.4} {code}")
    }
}

/// "rates from 3 days ago" once older than two days, otherwise the plain date.
pub fn subtitle(typed: &str, rates: &Rates, today: NaiveDate) -> String {
    let age = rates.age_days(today);
    if age > 2 {
        format!("{typed} · rates from {age} days ago")
    } else {
        format!("{typed} · ECB rates {}", rates.date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Rates {
        parse(include_str!("../../tests/fixtures/eurofxref-daily.xml"), 0).unwrap()
    }

    #[test]
    fn parses_fixture() {
        let r = fixture();
        assert_eq!(r.date, "2026-10-16");
        assert_eq!(r.per_eur["EUR"], 1.0);
        assert_eq!(r.per_eur["USD"], 1.1253);
        assert!(r.per_eur.len() > 25);
    }

    #[test]
    fn cross_rate() {
        let r = fixture();
        let want = 250.0 * 106.8640 / 1.1253;
        assert!((r.convert(250.0, "USD", "INR").unwrap() - want).abs() < 1e-9);
        assert!((r.convert(100.0, "EUR", "GBP").unwrap() - 84.385).abs() < 1e-9);
        assert_eq!(r.convert(1.0, "USD", "XXX"), None);
    }

    #[test]
    fn malformed_is_an_error() {
        assert!(parse("<a><b></a>", 0).is_err());
        assert_eq!(parse("", 0), Err(RatesError::Empty));
        assert_eq!(
            parse("<Cube time='2026-01-01'></Cube>", 0),
            Err(RatesError::Empty)
        );
    }

    #[test]
    fn asks() {
        let r = fixture();
        let ask = |t| parse_ask(t, &r);
        let inr = |a, f: &str| Ask {
            amount: a,
            from: f.into(),
            to: "INR".into(),
        };
        assert_eq!(ask("250 usd in inr"), Some(inr(250.0, "USD")));
        assert_eq!(ask("$20 in inr"), Some(inr(20.0, "USD")));
        assert_eq!(ask("$ 1,000.5 to Rupees"), Some(inr(1000.5, "USD")));
        assert_eq!(ask("20 dollars as INR"), Some(inr(20.0, "USD")));
        assert_eq!(ask("₹500 to usd").map(|a| a.to), Some("USD".into()));
        assert_eq!(ask("100 eur to gbp").map(|a| a.from), Some("EUR".into()));
        for bad in [
            "5pm ist in pst",
            "2.5 kg to lb",
            "250 usd in xyz",
            "$5 usd in inr",
            "usd in inr",
            "100 usd",
        ] {
            assert_eq!(ask(bad), None, "{bad}");
        }
    }

    #[test]
    fn subtitle_age() {
        let r = fixture();
        let d = |s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        assert!(subtitle("x", &r, d("2026-10-17")).contains("2026-10-16"));
        assert!(subtitle("x", &r, d("2026-10-19")).contains("rates from 3 days ago"));
    }
}
