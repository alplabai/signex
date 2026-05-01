//! Heuristic pinout extractor — datasheet PDF → guessed pin list.
//!
//! Per v0.9-library-plan.md §11.3 the **real** LLM-based symbol synthesis is deferred
//! to v3.x. This module provides a deliberately dumb, deterministic stub:
//!
//! 1. Run [`pdf_extract::extract_text_from_mem`] over the bytes.
//! 2. Walk the text line-by-line looking for "pinout table" rows of the
//!    shape `<pin#> <name> <type/description>`.
//! 3. Classify each row by keyword (`OUT*`, `IN*`, `GND/VCC`, …).
//! 4. Return a [`PinoutGuess`] with a confidence score in `[0, 1]`.
//!
//! Hard guarantees:
//! * **No network calls.**
//! * **No LLM API usage.**
//! * **No `reqwest` / `oauth2`.**
//!
//! Caller policy: the Component Editor shows a "Low confidence — review
//! manually" banner whenever `confidence < 0.5`.
//!
//! Feature gate: `ai-stub`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// One pin guessed from a datasheet table row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinGuess {
    /// Pin number as it appeared on the datasheet, e.g. `"1"`, `"A1"`.
    pub number: String,
    /// Pin name as it appeared on the datasheet, e.g. `"OUT"`, `"VCC"`.
    pub name: String,
    /// Coarse classification — `"input" | "output" | "power" | "passive" | "unknown"`.
    pub kind: String,
}

/// Result of the heuristic extractor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PinoutGuess {
    /// Pins recovered from the datasheet, in the order they were found.
    pub pins: Vec<PinGuess>,
    /// `[0, 1]` — caller should warn if `< 0.5`.
    pub confidence: f32,
}

/// Heuristic pinout extractor. **No LLM**, **no network**.
///
/// Returns an empty pin list with `confidence < 0.3` when:
/// * the PDF parser fails to extract any text;
/// * the extracted text contains no recognisable pin-table rows.
pub fn extract_pinout(pdf_bytes: &[u8]) -> PinoutGuess {
    let text = match pdf_extract::extract_text_from_mem(pdf_bytes) {
        Ok(t) => t,
        Err(_) => return PinoutGuess::default(),
    };
    extract_pinout_from_text(&text)
}

/// Pure-text variant — split out for unit-testing without a real PDF.
pub fn extract_pinout_from_text(text: &str) -> PinoutGuess {
    let mut pins = Vec::new();
    let mut seen_numbers = std::collections::HashSet::new();

    for line in text.lines() {
        if let Some(pin) = parse_pin_row(line) {
            // Dedupe by pin number so re-prints of the same table on a second
            // page don't double-count.
            if seen_numbers.insert(pin.number.clone()) {
                pins.push(pin);
            }
        }
    }

    let confidence = score_confidence(&pins, text);
    PinoutGuess { pins, confidence }
}

/// Match a single pin-table row.
///
/// Acceptable shapes (whitespace-tolerant):
/// * `1 ADJ Input` — number, name, type/description
/// * `1   ADJ   Adjustment terminal`
/// * `A1 VCC Power supply`
fn parse_pin_row(line: &str) -> Option<PinGuess> {
    let re = pin_row_regex();
    let caps = re.captures(line.trim())?;
    let number = caps.get(1)?.as_str().to_string();
    let name = caps.get(2)?.as_str().to_string();
    let rest = caps.get(3).map(|m| m.as_str()).unwrap_or("");

    // Reject obvious chatter — a real pin row has a SHORT name (≤ 12 chars)
    // and a number that looks like a pin id (digits, optional letter prefix).
    if name.len() > 12 {
        return None;
    }

    let kind = classify_pin_kind(&name, rest);
    Some(PinGuess { number, name, kind })
}

/// Classify a pin by name + description keywords.
fn classify_pin_kind(name: &str, description: &str) -> String {
    let n = name.to_ascii_uppercase();
    let d = description.to_ascii_lowercase();

    // Power rails — checked first because GND/VCC names don't look like I/O.
    if n == "GND" || n == "VSS" || n == "VEE" || n.starts_with("GND") {
        return "power".to_string();
    }
    if n == "VCC" || n == "VDD" || n == "VSS" || n.starts_with("V+") || n.starts_with("V-") {
        return "power".to_string();
    }
    if n.starts_with('V') && n.len() <= 5 {
        // VIN, VOUT, VREF, VBAT etc. — power-ish
        if n.contains("OUT") || d.contains("output") {
            return "output".to_string();
        }
        if n.contains("IN") || d.contains("input") {
            return "input".to_string();
        }
        return "power".to_string();
    }

    // Output — name or description
    if n == "OUT" || n == "Q" || n.starts_with("OUT") || n.ends_with("OUT") || d.contains("output")
    {
        return "output".to_string();
    }

    // Input — name or description (after output, since "OUT" wins over "IN"
    // when both appear).
    if n == "IN" || n.starts_with("IN") || n.ends_with("IN") || d.contains("input") {
        return "input".to_string();
    }

    // Passives — adjust, sense, ref, fb (feedback), etc. on linear regs.
    if n == "ADJ"
        || n == "FB"
        || n == "REF"
        || n == "SENSE"
        || d.contains("adjust")
        || d.contains("feedback")
        || d.contains("reference")
    {
        return "passive".to_string();
    }

    "unknown".to_string()
}

/// Compute confidence in `[0, 1]` from the parsed pin list and the source
/// text.
///
/// Heuristic — reward:
/// * having ≥ 2 pins (pinouts are never 1-pin)
/// * the source text containing pinout-y vocabulary
/// * pin numbers being a contiguous-ish sequence (1, 2, 3 …)
///
/// Penalise:
/// * empty pin list → near zero
/// * a single pin → cap at 0.3
fn score_confidence(pins: &[PinGuess], text: &str) -> f32 {
    if pins.is_empty() {
        // Tiny non-zero so callers can still tell "we ran, found nothing"
        // apart from "we never ran".
        return 0.0;
    }
    if pins.len() == 1 {
        return 0.25;
    }

    let mut score: f32 = 0.4;

    // Pin-count bonus: more pins == more table-like.
    score += (pins.len() as f32 / 16.0).min(0.2);

    // Vocabulary bonus — pinout tables almost always include words like
    // "pin", "function", "description", "name".
    let lower = text.to_ascii_lowercase();
    if lower.contains("pin") {
        score += 0.1;
    }
    if lower.contains("function") || lower.contains("description") {
        score += 0.1;
    }
    if lower.contains("pinout") || lower.contains("pin configuration") {
        score += 0.1;
    }

    // Sequence bonus — pin numbers 1, 2, 3, … is a strong signal.
    let mut numeric: Vec<u32> = pins
        .iter()
        .filter_map(|p| p.number.parse::<u32>().ok())
        .collect();
    numeric.sort_unstable();
    if numeric.len() >= 2 {
        let starts_at_one = numeric[0] == 1;
        let contiguous = numeric.windows(2).all(|w| w[1] == w[0] + 1);
        if starts_at_one && contiguous {
            score += 0.2;
        } else if contiguous {
            score += 0.1;
        }
    }

    score.clamp(0.0, 1.0)
}

/// Lazily compiled regex matching a pin-table row.
///
/// Capture groups:
/// 1. pin number (digits, optional letter-prefix like `A1`)
/// 2. pin name (letters, digits, `+`, `-`, `_`, `/`)
/// 3. remainder of the line (description / type)
fn pin_row_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([A-Z]?\d{1,3})\s+([A-Za-z][A-Za-z0-9+\-_/]{0,11})\s+(.{1,80})$")
            .expect("static pin-row regex must compile")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lm317_style_table_from_plain_text() {
        let text = "\
LM317 Pin Configuration
Pin  Name  Function
1    ADJ   Adjustment terminal
2    OUT   Regulated output voltage
3    IN    Unregulated input voltage
";
        let guess = extract_pinout_from_text(text);
        assert_eq!(guess.pins.len(), 3, "should find 3 pins");
        let names: Vec<&str> = guess.pins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"ADJ"));
        assert!(names.contains(&"OUT"));
        assert!(names.contains(&"IN"));
        assert!(
            guess.confidence >= 0.7,
            "LM317 confidence {} should be ≥ 0.7",
            guess.confidence
        );
    }

    #[test]
    fn empty_text_yields_zero_confidence() {
        let guess = extract_pinout_from_text("");
        assert!(guess.pins.is_empty());
        assert!(guess.confidence < 0.3);
    }

    #[test]
    fn garbage_text_yields_low_confidence() {
        let text = "\
This is a marketing brochure for our exciting new product line.
Order today and save 20%! Lorem ipsum dolor sit amet, consectetur.
Visit our website for more information about quality and reliability.
";
        let guess = extract_pinout_from_text(text);
        assert!(
            guess.confidence < 0.3,
            "garbage text confidence {} should be < 0.3 (pins={:?})",
            guess.confidence,
            guess.pins
        );
    }

    #[test]
    fn classifies_power_input_output_passive() {
        assert_eq!(classify_pin_kind("VCC", ""), "power");
        assert_eq!(classify_pin_kind("GND", ""), "power");
        assert_eq!(classify_pin_kind("OUT", "Regulated output"), "output");
        assert_eq!(classify_pin_kind("IN", "Unregulated input"), "input");
        assert_eq!(classify_pin_kind("ADJ", "Adjustment"), "passive");
    }

    #[test]
    fn rejects_long_garbage_token_as_pin_name() {
        // 13+ char "name" should be rejected.
        let row = "1 ThisIsTooLongAName more words";
        assert!(parse_pin_row(row).is_none());
    }

    #[test]
    fn dedupes_repeated_pin_numbers() {
        let text = "\
1 ADJ Adjustment terminal
2 OUT Output
1 ADJ Adjustment terminal again on next page
3 IN Input
";
        let guess = extract_pinout_from_text(text);
        assert_eq!(guess.pins.len(), 3, "should dedupe pin 1");
    }

    #[test]
    fn pinout_guess_round_trips_via_serde() {
        let g = PinoutGuess {
            pins: vec![PinGuess {
                number: "1".into(),
                name: "ADJ".into(),
                kind: "passive".into(),
            }],
            confidence: 0.42,
        };
        let json = serde_json::to_string(&g).unwrap();
        let back: PinoutGuess = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }
}
