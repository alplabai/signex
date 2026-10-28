//! Electrical units and SI prefixes, and the alias tables that read them.

use std::fmt;

/// A physical quantity kind a component value can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Ohm,
    Farad,
    Henry,
    Volt,
    Ampere,
    Watt,
    Hertz,
    Percent,
    Ppm,
    Celsius,
    Second,
}

impl Unit {
    /// Every unit, in declaration order.
    pub const ALL: [Unit; 11] = [
        Unit::Ohm,
        Unit::Farad,
        Unit::Henry,
        Unit::Volt,
        Unit::Ampere,
        Unit::Watt,
        Unit::Hertz,
        Unit::Percent,
        Unit::Ppm,
        Unit::Celsius,
        Unit::Second,
    ];

    /// The canonical display symbol (`Ω`, `F`, `°C`, ...).
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Ohm => "Ω",
            Unit::Farad => "F",
            Unit::Henry => "H",
            Unit::Volt => "V",
            Unit::Ampere => "A",
            Unit::Watt => "W",
            Unit::Hertz => "Hz",
            Unit::Percent => "%",
            Unit::Ppm => "ppm",
            Unit::Celsius => "°C",
            Unit::Second => "s",
        }
    }

    /// `false` for `%`, `ppm` and `°C`, which are never SI-prefixed.
    pub fn takes_prefix(self) -> bool {
        !matches!(self, Unit::Percent | Unit::Ppm | Unit::Celsius)
    }

    /// Resolve a whole unit token (no prefix). Symbols are case-sensitive
    /// (`F` is farad); word aliases (`ohm`, `hz`, `degC`) are not.
    pub fn from_alias(token: &str) -> Option<Unit> {
        let unit = match token {
            "Ω" | "\u{2126}" => Unit::Ohm,
            "F" => Unit::Farad,
            "H" => Unit::Henry,
            "V" => Unit::Volt,
            "A" => Unit::Ampere,
            "W" => Unit::Watt,
            "%" => Unit::Percent,
            "°C" | "\u{2103}" | "C" => Unit::Celsius,
            "s" => Unit::Second,
            "Hz" => Unit::Hertz,
            _ => return Unit::from_word(token),
        };
        Some(unit)
    }

    fn from_word(token: &str) -> Option<Unit> {
        match token.to_ascii_lowercase().as_str() {
            "ohm" | "ohms" => Some(Unit::Ohm),
            "hz" => Some(Unit::Hertz),
            "ppm" => Some(Unit::Ppm),
            "degc" | "celsius" => Some(Unit::Celsius),
            _ => None,
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// A decimal SI prefix, `p` through `G`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SiPrefix {
    Pico,
    Nano,
    Micro,
    Milli,
    /// No prefix (10^0).
    One,
    Kilo,
    Mega,
    Giga,
}

impl SiPrefix {
    /// Ascending by exponent.
    pub const ALL: [SiPrefix; 8] = [
        SiPrefix::Pico,
        SiPrefix::Nano,
        SiPrefix::Micro,
        SiPrefix::Milli,
        SiPrefix::One,
        SiPrefix::Kilo,
        SiPrefix::Mega,
        SiPrefix::Giga,
    ];

    /// Power of ten this prefix stands for.
    pub fn exponent(self) -> i32 {
        match self {
            SiPrefix::Pico => -12,
            SiPrefix::Nano => -9,
            SiPrefix::Micro => -6,
            SiPrefix::Milli => -3,
            SiPrefix::One => 0,
            SiPrefix::Kilo => 3,
            SiPrefix::Mega => 6,
            SiPrefix::Giga => 9,
        }
    }

    /// Display symbol; `µ` is U+00B5. Empty for [`SiPrefix::One`].
    pub fn symbol(self) -> &'static str {
        match self {
            SiPrefix::Pico => "p",
            SiPrefix::Nano => "n",
            SiPrefix::Micro => "µ",
            SiPrefix::Milli => "m",
            SiPrefix::One => "",
            SiPrefix::Kilo => "k",
            SiPrefix::Mega => "M",
            SiPrefix::Giga => "G",
        }
    }

    /// The prefixes a picker offers for `unit`: all of them, or only
    /// [`SiPrefix::One`] for `%`, `ppm` and `°C`.
    pub fn all_for(unit: Unit) -> &'static [SiPrefix] {
        if unit.takes_prefix() {
            &SiPrefix::ALL
        } else {
            &[SiPrefix::One]
        }
    }

    /// Read one prefix character: `p n µ μ u m k K M G`. Case matters
    /// (`m` milli, `M` mega); `K` is accepted as kilo because stored libraries contain `KΩ`.
    pub fn from_char(c: char) -> Option<SiPrefix> {
        Some(match c {
            'p' => SiPrefix::Pico,
            'n' => SiPrefix::Nano,
            'µ' | 'μ' | 'u' => SiPrefix::Micro,
            'm' => SiPrefix::Milli,
            'k' | 'K' => SiPrefix::Kilo,
            'M' => SiPrefix::Mega,
            'G' => SiPrefix::Giga,
            _ => return None,
        })
    }

    /// `mantissa` expressed in the base unit. Negative exponents divide by
    /// an exactly representable power of ten, so `4.7 µ` stays `4.7e-6`.
    pub fn scale(self, mantissa: f64) -> f64 {
        match self.exponent() {
            e if e >= 0 => mantissa * 10f64.powi(e),
            e => mantissa / 10f64.powi(-e),
        }
    }

    /// Inverse of [`SiPrefix::scale`]: a base-unit value as a mantissa.
    pub fn unscale(self, base: f64) -> f64 {
        match self.exponent() {
            e if e >= 0 => base / 10f64.powi(e),
            e => base * 10f64.powi(-e),
        }
    }
}
