//! Schematic values with units: `4.7µH`, `22 uF`, `5mΩ`, `500kHz`, a bare
//! `10k` on a resistor, `4k7` and `2R2`. A cell receives each value it reads
//! as a [`Quantity`], so its arithmetic keeps the units (mockup row 200).

use serde::{Deserialize, Serialize};

/// A unit a schematic value may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Ohm,
    Farad,
    Henry,
    Hertz,
    Volt,
    Ampere,
    Watt,
    Second,
    /// A plain number, such as a ratio or a count.
    One,
}

impl Unit {
    /// The symbol the page prints and the runtime's units library reads.
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Ohm => "Ω",
            Unit::Farad => "F",
            Unit::Henry => "H",
            Unit::Hertz => "Hz",
            Unit::Volt => "V",
            Unit::Ampere => "A",
            Unit::Watt => "W",
            Unit::Second => "s",
            Unit::One => "",
        }
    }

    /// The unit a value with none written takes on a part with designator
    /// `designator`: ohms on an `R`, farads on a `C`, henries on an `L`.
    pub fn implied_by(designator: &str) -> Unit {
        let prefix: String = designator
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_uppercase();
        match prefix.as_str() {
            "R" | "RN" | "RV" => Unit::Ohm,
            "C" => Unit::Farad,
            "L" | "FB" => Unit::Henry,
            _ => Unit::One,
        }
    }

    /// The unit written as `text` (a symbol or a word), if it is one.
    fn parse(text: &str) -> Option<Unit> {
        let unit = match text {
            // The Greek capital omega and the ohm sign.
            "\u{3a9}" | "\u{2126}" | "ohm" | "Ohm" | "ohms" | "R" => Unit::Ohm,
            "F" => Unit::Farad,
            "H" => Unit::Henry,
            "Hz" | "hz" => Unit::Hertz,
            "V" => Unit::Volt,
            "A" => Unit::Ampere,
            "W" => Unit::Watt,
            "s" => Unit::Second,
            "" => Unit::One,
            _ => return None,
        };
        Some(unit)
    }
}

/// A value and its unit, the magnitude in the unit without a prefix
/// (4.7 µH is `4.7e-6` henry).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub magnitude: f64,
    pub unit: Unit,
}

/// The SI prefixes a value is read and printed with, smallest first.
const PREFIXES: [(&str, i32); 9] = [
    ("p", -12),
    ("n", -9),
    ("µ", -6),
    ("m", -3),
    ("", 0),
    ("k", 3),
    ("M", 6),
    ("G", 9),
    ("T", 12),
];

impl Quantity {
    /// `text`, a schematic value, as a quantity; a value written without a
    /// unit takes `implied`. `None` when it is not a number with an optional
    /// prefix and unit (`DNP`, `BAT54`).
    pub fn parse(text: &str, implied: Unit) -> Option<Quantity> {
        let text = text.trim();
        let number_end = text
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
            .unwrap_or(text.len());
        let (number, rest) = text.split_at(number_end);
        if number.is_empty() || number == "-" || number == "+" {
            return None;
        }
        let rest = rest.trim_start();
        if let Some((marker, tail)) = split_marker(rest)
            && !number.contains('.')
        {
            return Quantity::marked(number, marker, tail, implied);
        }
        let magnitude: f64 = number.parse().ok()?;
        let (exponent, unit) = prefix_and_unit(rest, implied)?;
        Some(Quantity::scaled(magnitude, exponent, unit))
    }

    /// `4k7`, `2R2`, `4u7`: the prefix (or `R`) `marker` stands for the
    /// point between `whole` and the digits that open `tail`.
    fn marked(whole: &str, marker: &str, tail: &str, implied: Unit) -> Option<Quantity> {
        let digits_end = tail
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(tail.len());
        let (digits, unit_text) = tail.split_at(digits_end);
        let magnitude: f64 = format!("{whole}.{digits}").parse().ok()?;
        let exponent = if marker == "R" {
            0
        } else {
            prefix_exponent(marker)?
        };
        let unit = match unit_text.trim() {
            "" if marker == "R" => Unit::Ohm,
            "" => implied,
            written => Unit::parse(written)?,
        };
        Some(Quantity::scaled(magnitude, exponent, unit))
    }

    fn scaled(magnitude: f64, exponent: i32, unit: Unit) -> Quantity {
        Quantity {
            magnitude: magnitude * 10f64.powi(exponent),
            unit,
        }
    }

    /// The quantity in engineering form, three significant digits at most:
    /// `4.7 µH`, `5 mΩ`, `500 kHz`, `22 µF`. A plain number prints without a
    /// unit.
    pub fn display(&self) -> String {
        let symbol = self.unit.symbol();
        let join = |number: String, prefix: &str| {
            let unit = format!("{prefix}{symbol}");
            if unit.is_empty() {
                number
            } else {
                format!("{number} {unit}")
            }
        };
        let magnitude = self.magnitude;
        if magnitude == 0.0 || !magnitude.is_finite() {
            return join(format!("{magnitude}"), "");
        }
        let decade = magnitude.abs().log10().floor() as i32;
        let mut index = PREFIXES
            .iter()
            .rposition(|&(_, exponent)| exponent <= decade)
            .unwrap_or(0);
        let mut scaled = magnitude / 10f64.powi(PREFIXES[index].1);
        let mut rounded = round_significant(scaled, 3);
        if rounded.abs() >= 1000.0 && index + 1 < PREFIXES.len() {
            index += 1;
            scaled = magnitude / 10f64.powi(PREFIXES[index].1);
            rounded = round_significant(scaled, 3);
        }
        join(trim_number(rounded), PREFIXES[index].0)
    }
}

/// A one-letter prefix or `R` that opens `rest` and is followed by a digit.
fn split_marker(rest: &str) -> Option<(&str, &str)> {
    let first = rest.chars().next()?;
    let len = first.len_utf8();
    let tail = &rest[len..];
    let marker = &rest[..len];
    let is_marker = marker == "R" || prefix_exponent(marker).is_some();
    (is_marker && tail.starts_with(|c: char| c.is_ascii_digit())).then_some((marker, tail))
}

/// The exponent of an SI prefix; `u` and the Greek mu stand for micro too.
fn prefix_exponent(prefix: &str) -> Option<i32> {
    match prefix {
        "u" | "\u{3bc}" => Some(-6),
        "K" => Some(3),
        _ => PREFIXES
            .iter()
            .find(|&&(written, _)| written == prefix && !written.is_empty())
            .map(|&(_, exponent)| exponent),
    }
}

/// `rest`, what follows the number, as a prefix and a unit. A unit alone
/// wins over a prefix of the same letter (`m` is milli, `M` mega, but `F`
/// is farads and `H` henries).
fn prefix_and_unit(rest: &str, implied: Unit) -> Option<(i32, Unit)> {
    let rest = rest.trim();
    if let Some(unit) = Unit::parse(rest) {
        let unit = if unit == Unit::One { implied } else { unit };
        return Some((0, unit));
    }
    let first = rest.chars().next()?;
    let (prefix, unit_text) = rest.split_at(first.len_utf8());
    let exponent = prefix_exponent(prefix)?;
    let unit = match Unit::parse(unit_text.trim())? {
        Unit::One => implied,
        unit => unit,
    };
    Some((exponent, unit))
}

fn round_significant(value: f64, digits: i32) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    let decade = value.abs().log10().floor() as i32;
    let factor = 10f64.powi(digits - 1 - decade);
    (value * factor).round() / factor
}

/// `value` with no trailing zeros after the point: `4.7`, `500`, `16.7`.
fn trim_number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}
