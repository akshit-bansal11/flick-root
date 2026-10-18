//! Math and unit evaluation through fend-core, with a pre-filter and a hard time limit.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Inputs longer than this are never math.
const MAX_LEN: usize = 200;
/// Evaluation budget; fend aborts when it is exceeded.
const BUDGET: Duration = Duration::from_millis(if cfg!(debug_assertions) { 50 } else { 5 });
/// Warm-up limit; runs once, off the keystroke path.
const WARMUP: Duration = Duration::from_secs(2);

/// Set once the process-wide first-use setup (unit tables) has run.
static READY: AtomicBool = AtomicBool::new(false);

/// Pays fend's first-use cost (about 1 ms in release) so keystroke evaluations never do.
/// Call from a blocking thread at feature start. Uses a throwaway context and preview mode,
/// so no `_`/`ans` value leaks into later evaluations.
pub fn warm() {
    let ctx = fend_core::Context::new();
    let int = Deadline(Instant::now() + WARMUP);
    let _ = catch_unwind(AssertUnwindSafe(|| {
        fend_core::evaluate_preview_with_interrupt("1 kg to lb", &ctx, &int)
    }));
    READY.store(true, Ordering::Release);
}

/// Currency words and symbols; fend cannot convert them, so `rates.rs` answers them instead.
const CURRENCY_WORDS: &[&str] = &[
    "usd", "eur", "inr", "gbp", "jpy", "cad", "aud", "chf", "cny", "dollar", "dollars", "euro",
    "euros", "rupee", "rupees",
];

thread_local! {
    /// Per-thread context (about 14 us to build); preview evaluation never mutates it.
    static CTX: fend_core::Context = fend_core::Context::new();
}

/// Interrupt that fires once the instant has passed.
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
    // Not warmed yet: skip this keystroke rather than block on setup.
    if !READY.load(Ordering::Acquire) {
        return None;
    }
    let int = Deadline(Instant::now() + BUDGET);
    let res = catch_unwind(AssertUnwindSafe(|| {
        CTX.with(|ctx| fend_core::evaluate_preview_with_interrupt(t, ctx, &int))
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
    fn warm_up_leaves_no_ans() {
        warm();
        for s in ["ans*2", "_+1", "ans"] {
            let r = evaluate(s);
            assert!(r.as_deref().is_none_or(|r| !r.contains("lb")), "{s}: {r:?}");
        }
    }

    #[test]
    fn units_and_arithmetic() {
        warm();
        assert!(evaluate("2.5 kg to lb").is_some_and(|r| r.starts_with("5.51")));
        assert_eq!(evaluate("2+2*3").as_deref(), Some("8"));
    }

    #[test]
    fn rejects_non_math() {
        warm();
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
        warm();
        assert_eq!(evaluate(&"1+".repeat(500)), None);
        let t = Instant::now();
        let _ = evaluate("9^9^9^9^9^9^9^9");
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
