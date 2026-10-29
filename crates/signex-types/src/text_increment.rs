//! Stepping the trailing number of a text: `D0` -> `D1` -> `D2`.
//!
//! The one rule behind every auto-increment (repeated label placement, Paste
//! Array, repeat last item), so they cannot drift apart.

/// Smallest and largest step the "Increment step" preference accepts.
pub const STEP_MIN: i32 = -100;
pub const STEP_MAX: i32 = 100;
/// The step out of the box.
pub const STEP_DEFAULT: i32 = 1;

/// A stored or typed step made valid: clamped to `STEP_MIN..=STEP_MAX`, and
/// `0` (which would stamp the same text forever) becomes the default.
pub fn clamp_step(step: i64) -> i32 {
    match step.clamp(i64::from(STEP_MIN), i64::from(STEP_MAX)) {
        0 => STEP_DEFAULT,
        valid => valid as i32,
    }
}

/// `text` with its trailing decimal number moved by `step`.
///
/// Only a number at the very end counts (`D0`, `A07`, `12`); `DATA[3]` has
/// none and comes back unchanged. A number written with leading zeros keeps
/// its width (`A07` -> `A08`, `A09` -> `A10`); one without grows freely
/// (`D9` -> `D10`). A result below zero, or a number too long to hold,
/// leaves the text unchanged.
pub fn increment_trailing_number(text: &str, step: i64) -> String {
    let digits_at = text.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| {
        i + text[i..].chars().next().map_or(1, char::len_utf8)
    });
    let digits = &text[digits_at..];
    let Ok(value) = digits.parse::<u64>() else {
        return text.to_owned();
    };
    let Some(next) = i64::try_from(value)
        .ok()
        .and_then(|v| v.checked_add(step))
        .filter(|n| *n >= 0)
    else {
        return text.to_owned();
    };
    let padded = digits.len() > 1 && digits.starts_with('0');
    let width = if padded { digits.len() } else { 0 };
    format!("{}{next:0width$}", &text[..digits_at])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn up(text: &str) -> String {
        increment_trailing_number(text, 1)
    }

    #[test]
    fn a_trailing_number_steps_by_one() {
        assert_eq!(up("D0"), "D1");
        assert_eq!(up("D9"), "D10");
        assert_eq!(up("NET_12"), "NET_13");
        assert_eq!(up("7"), "8");
    }

    #[test]
    fn zero_padding_is_kept() {
        assert_eq!(up("A07"), "A08");
        assert_eq!(up("A09"), "A10");
        assert_eq!(up("A99"), "A100");
        assert_eq!(increment_trailing_number("D010", -1), "D009");
        assert_eq!(increment_trailing_number("D10", -1), "D9");
    }

    #[test]
    fn text_not_ending_in_digits_is_unchanged() {
        assert_eq!(up("DATA[3]"), "DATA[3]");
        assert_eq!(up("CLK"), "CLK");
        assert_eq!(up(""), "");
    }

    #[test]
    fn only_the_last_number_moves() {
        assert_eq!(up("U2_D5"), "U2_D6");
        assert_eq!(up("\u{e9}3"), "\u{e9}4");
    }

    #[test]
    fn a_step_can_be_negative_but_not_below_zero() {
        assert_eq!(increment_trailing_number("D5", -2), "D3");
        assert_eq!(increment_trailing_number("D1", -1), "D0");
        assert_eq!(increment_trailing_number("D1", -2), "D1");
        assert_eq!(increment_trailing_number("D1", 100), "D101");
    }

    #[test]
    fn a_number_too_long_to_hold_is_unchanged() {
        let long = "X99999999999999999999999";
        assert_eq!(up(long), long);
        let edge = format!("X{}", i64::MAX);
        assert_eq!(up(&edge), edge);
    }

    #[test]
    fn a_step_is_clamped_and_never_zero() {
        assert_eq!(clamp_step(0), STEP_DEFAULT);
        assert_eq!(clamp_step(5), 5);
        assert_eq!(clamp_step(-3), -3);
        assert_eq!(clamp_step(10_000), STEP_MAX);
        assert_eq!(clamp_step(-10_000), STEP_MIN);
    }
}
