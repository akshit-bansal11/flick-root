//! Math and unit evaluation through fend-core, with a pre-filter and a hard time limit.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

/// Inputs longer than this are never math.
const MAX_LEN: usize = 200;
/// Evaluation budget; fend aborts when it is exceeded.
const BUDGET: Duration = Duration::from_millis(5);

/// Currency words and symbols; those expressions wait for the rates feature (T4.4).
const CURRENCY_WORDS: &[&str] = &[
    "usd", "eur", "inr", "gbp", "jpy", "cad", "aud", "chf", "cny", "dollar", "dollars", "euro",
    "euros", "rupee", "rupees",
];

thread_local! {
    /// Context built once per thread.
    static TEMPLATE: fend_core::Context = fend_core::Context::new();
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

/// Cheap reject: needs a digit plus an operator or a letter (unit, "to"/"in"/"as", function).
fn plausible(s: &str) -> bool {
    s.len() <= MAX_LEN
        && s.bytes().any(|b| b.is_ascii_digit())
        && s.chars()
            .any(|c| c.is_alphabetic() || "+-*/^%()!".contains(c))
}

/// True when the text mentions a currency; handled elsewhere.
fn is_currency(s: &str) -> bool {
    s.chars().any(|c| "$€₹£¥".contains(c))
        || s.to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .any(|w| CURRENCY_WORDS.contains(&w))
}

/// Evaluates `text`; returns the display result, or `None` when it is not math, fails, or only echoes the input.
pub fn evaluate(text: &str) -> Option<String> {
    let t = text.trim();
    if !plausible(t) || is_currency(t) {
        return None;
    }
    let int = Deadline(Instant::now() + BUDGET);
    // Preview evaluation never mutates the context, so one per thread is shared and variables cannot persist.
    let res = catch_unwind(AssertUnwindSafe(|| {
        TEMPLATE.with(|ctx| fend_core::evaluate_preview_with_interrupt(t, ctx, &int))
    }))
    .ok()?;
    let out = res.get_main_result().trim();
    let out = out.strip_prefix("approx. ").unwrap_or(out);
    if out.is_empty() || out == t || out.contains('\n') {
        return None;
    }
    Some(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_arithmetic() {
        assert!(evaluate("2.5 kg to lb").is_some_and(|r| r.starts_with("5.51")));
        assert_eq!(evaluate("2+2*3").as_deref(), Some("8"));
    }

    #[test]
    fn rejects_non_math() {
        for s in [
            "hello",
            "notepad",
            "2++",
            "",
            "42",
            "$5 to eur",
            "5 usd in inr",
        ] {
            assert_eq!(evaluate(s), None, "{s}");
        }
    }

    #[test]
    fn long_and_heavy_inputs_are_bounded() {
        assert_eq!(evaluate(&"1+".repeat(500)), None);
        let t = Instant::now();
        let _ = evaluate("9^9^9^9^9^9^9^9");
        assert!(t.elapsed() < Duration::from_millis(500));
    }
}
