//! Time-zone conversion: "5pm ist in pst", "17:30 utc to cet", "now in tokyo".

use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Abbreviation or city to IANA zone. Abbreviations are ambiguous in the wild; choices made:
/// IST is India (not Irish or Israel), CST is US Central (not China), BST is British Summer Time.
/// Zones follow DST, so "pst" in July gives Pacific daylight time.
const ZONES: &[(&str, Tz)] = &[
    ("ist", Tz::Asia__Kolkata),
    ("pst", Tz::America__Los_Angeles),
    ("pdt", Tz::America__Los_Angeles),
    ("mst", Tz::America__Denver),
    ("mdt", Tz::America__Denver),
    ("cst", Tz::America__Chicago),
    ("cdt", Tz::America__Chicago),
    ("est", Tz::America__New_York),
    ("edt", Tz::America__New_York),
    ("cet", Tz::Europe__Paris),
    ("cest", Tz::Europe__Paris),
    ("eet", Tz::Europe__Athens),
    ("eest", Tz::Europe__Athens),
    ("gmt", Tz::GMT),
    ("utc", Tz::UTC),
    ("bst", Tz::Europe__London),
    ("jst", Tz::Asia__Tokyo),
    ("kst", Tz::Asia__Seoul),
    ("sgt", Tz::Asia__Singapore),
    ("hkt", Tz::Asia__Hong_Kong),
    ("aest", Tz::Australia__Sydney),
    ("aedt", Tz::Australia__Sydney),
    ("nzst", Tz::Pacific__Auckland),
    ("tokyo", Tz::Asia__Tokyo),
    ("london", Tz::Europe__London),
    ("paris", Tz::Europe__Paris),
    ("berlin", Tz::Europe__Berlin),
    ("delhi", Tz::Asia__Kolkata),
    ("mumbai", Tz::Asia__Kolkata),
    ("sydney", Tz::Australia__Sydney),
    ("dubai", Tz::Asia__Dubai),
    ("singapore", Tz::Asia__Singapore),
];

fn zone(w: &str) -> Option<Tz> {
    ZONES.iter().find(|(k, _)| *k == w).map(|(_, z)| *z)
}

/// Parses "5pm", "5:30pm", "17:30", with an optional separate "pm" token. Returns the time and tokens used.
fn parse_time(toks: &[&str]) -> Option<(NaiveTime, usize)> {
    let first = *toks.first()?;
    let (body, mut pm) = match first.strip_suffix("pm") {
        Some(b) => (b, Some(true)),
        None => match first.strip_suffix("am") {
            Some(b) => (b, Some(false)),
            None => (first, None),
        },
    };
    let mut used = 1;
    if pm.is_none()
        && let Some(&m) = toks.get(1)
        && (m == "am" || m == "pm")
    {
        pm = Some(m == "pm");
        used = 2;
    }
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(p) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            h % 12 + if p { 12 } else { 0 }
        }
        None => h,
    };
    Some((NaiveTime::from_hms_opt(h, m, 0)?, used))
}

/// Converts a zone expression using `now` as the current instant; `None` when it is not one.
pub fn convert(text: &str, now: DateTime<Utc>) -> Option<String> {
    let lower = text.trim().to_lowercase();
    let toks: Vec<&str> = lower.split_whitespace().collect();
    if toks.len() < 3 || toks.len() > 6 {
        return None;
    }
    let sep = toks.iter().position(|t| *t == "in" || *t == "to")?;
    if toks.len() != sep + 2 {
        return None;
    }
    let dst_word = toks[sep + 1];
    let dst = zone(dst_word)?;
    let (instant, src_date) = if toks[..sep] == ["now"] {
        (now, now.date_naive())
    } else {
        let (time, used) = parse_time(&toks[..sep])?;
        let src = match &toks[used..sep] {
            [z] => zone(z)?,
            [] => Tz::UTC,
            _ => return None,
        };
        let date = now.with_timezone(&src).date_naive();
        let at = src.from_local_datetime(&date.and_time(time)).earliest()?;
        (at.with_timezone(&Utc), date)
    };
    let out = instant.with_timezone(&dst);
    let day = match out.date_naive().signed_duration_since(src_date).num_days() {
        d if d > 0 => " (next day)",
        d if d < 0 => " (previous day)",
        _ => "",
    };
    Some(format!(
        "{}{} {}",
        out.format("%-I:%M %p"),
        day,
        dst_word.to_uppercase()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(month: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, month, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn ist_to_pst_winter() {
        // 17:00 IST = 11:30 UTC = 03:30 PST, same calendar day.
        assert_eq!(
            convert("5pm ist in pst", at(1)).as_deref(),
            Some("3:30 AM PST")
        );
    }

    #[test]
    fn dst_applies_in_summer() {
        assert_eq!(
            convert("5pm ist in pst", at(7)).as_deref(),
            Some("4:30 AM PST")
        );
    }

    #[test]
    fn day_shift_and_now() {
        assert_eq!(
            convert("17:30 utc to jst", at(1)).as_deref(),
            Some("2:30 AM (next day) JST")
        );
        assert_eq!(
            convert("now in tokyo", at(1)).as_deref(),
            Some("9:00 PM TOKYO")
        );
    }

    #[test]
    fn rejects_garbage() {
        for s in [
            "hello",
            "5pm xyz in pst",
            "25pm ist in pst",
            "5pm ist in",
            "2 kg to lb",
        ] {
            assert_eq!(convert(s, at(1)), None, "{s}");
        }
    }
}
