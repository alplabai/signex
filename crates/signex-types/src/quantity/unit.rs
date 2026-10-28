//! The closed set of units a component value can carry.

use std::fmt;

use super::prefix::SiPrefix;

use SiPrefix::{Giga, Kilo, Mega, Micro, Milli, Nano, One, Pico};

/// A unit. Each has a canonical on-disk [`token`](Unit::token) equal to
/// the spelling class templates use today, a display
/// [`glyph`](Unit::glyph), and the prefixes a picker offers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Ohm,
    Farad,
    Henry,
    Volt,
    Ampere,
    Watt,
    Hertz,
    Percent,
    PpmPerKelvin,
    VoltPerMicrosecond,
    Awg,
    /// A unit string this build does not know; kept as written, no prefix.
    Other(String),
}

impl Unit {
    /// Every known unit, for a unit picker.
    pub const STANDARD: [Unit; 11] = [
        Unit::Ohm,
        Unit::Farad,
        Unit::Henry,
        Unit::Volt,
        Unit::Ampere,
        Unit::Watt,
        Unit::Hertz,
        Unit::Percent,
        Unit::PpmPerKelvin,
        Unit::VoltPerMicrosecond,
        Unit::Awg,
    ];

    /// Canonical on-disk spelling (`ohm`, `F`, `ppm/C`, `V/us`, ...).
    pub fn token(&self) -> &str {
        match self {
            Unit::Ohm => "ohm",
            Unit::Farad => "F",
            Unit::Henry => "H",
            Unit::Volt => "V",
            Unit::Ampere => "A",
            Unit::Watt => "W",
            Unit::Hertz => "Hz",
            Unit::Percent => "%",
            Unit::PpmPerKelvin => "ppm/C",
            Unit::VoltPerMicrosecond => "V/us",
            Unit::Awg => "AWG",
            Unit::Other(s) => s,
        }
    }

    /// Display glyph: `Ω` (U+03A9), `ppm/°C`, `V/µs` (U+00B5).
    pub fn glyph(&self) -> &str {
        match self {
            Unit::Ohm => "Ω",
            Unit::PpmPerKelvin => "ppm/°C",
            Unit::VoltPerMicrosecond => "V/µs",
            other => other.token(),
        }
    }

    /// Pure-ASCII spelling, used by the ASCII output profile.
    pub fn ascii(&self) -> &str {
        self.token()
    }

    /// Prefixes a picker offers, ascending. Single-element (`[One]`) for
    /// units that take none. A capacitance never offers `m`, so 4.7e-3 F
    /// shows as `4700 µF`.
    pub fn allowed_prefixes(&self) -> &'static [SiPrefix] {
        match self {
            Unit::Farad => &[Pico, Nano, Micro, One],
            Unit::Ohm => &[Milli, One, Kilo, Mega, Giga],
            Unit::Henry => &[Nano, Micro, Milli, One],
            Unit::Volt => &[Micro, Milli, One, Kilo],
            Unit::Ampere => &[Nano, Micro, Milli, One],
            Unit::Watt => &[Micro, Milli, One, Kilo, Mega],
            Unit::Hertz => &[One, Kilo, Mega, Giga],
            _ => &[One],
        }
    }

    /// `false` for `%`, `ppm/C`, `V/us`, `AWG` and unknown units.
    pub fn takes_prefix(&self) -> bool {
        self.allowed_prefixes().len() > 1
    }

    /// Resolve a whole unit token (no prefix) to a known unit. Glyphs are
    /// case-sensitive (`F` is farad); word spellings are not (`ohm`,
    /// `Ohms`, `hz`, `ppm/C`, `V/us`). Accepts `Ω` as U+03A9 or U+2126
    /// and `µ` as U+00B5 or U+03BC.
    pub fn from_alias(token: &str) -> Option<Unit> {
        let unit = match token {
            "Ω" | "\u{2126}" => Unit::Ohm,
            "F" => Unit::Farad,
            "H" => Unit::Henry,
            "V" => Unit::Volt,
            "A" => Unit::Ampere,
            "W" => Unit::Watt,
            "Hz" => Unit::Hertz,
            "%" => Unit::Percent,
            _ => return Unit::from_word(token),
        };
        Some(unit)
    }

    fn from_word(token: &str) -> Option<Unit> {
        match token.to_lowercase().as_str() {
            "ohm" | "ohms" => Some(Unit::Ohm),
            "hz" => Some(Unit::Hertz),
            "ppm/c" | "ppm/°c" => Some(Unit::PpmPerKelvin),
            "v/us" | "v/µs" | "v/μs" => Some(Unit::VoltPerMicrosecond),
            "awg" => Some(Unit::Awg),
            _ => None,
        }
    }

    /// [`Unit::from_alias`], or [`Unit::Other`] holding `token` as written.
    pub fn from_token_lossy(token: &str) -> Unit {
        Unit::from_alias(token).unwrap_or_else(|| Unit::Other(token.to_string()))
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.glyph())
    }
}
