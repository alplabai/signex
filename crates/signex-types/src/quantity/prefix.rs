//! SI prefixes `f` through `T`.

use super::unit::Unit;

/// A decimal SI prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SiPrefix {
    Femto,
    Pico,
    Nano,
    Micro,
    Milli,
    /// No prefix (10^0).
    One,
    Kilo,
    Mega,
    Giga,
    Tera,
}

impl SiPrefix {
    /// Ascending by exponent.
    pub const ALL: [SiPrefix; 10] = [
        SiPrefix::Femto,
        SiPrefix::Pico,
        SiPrefix::Nano,
        SiPrefix::Micro,
        SiPrefix::Milli,
        SiPrefix::One,
        SiPrefix::Kilo,
        SiPrefix::Mega,
        SiPrefix::Giga,
        SiPrefix::Tera,
    ];

    /// Power of ten this prefix stands for.
    pub fn exponent(self) -> i32 {
        match self {
            SiPrefix::Femto => -15,
            SiPrefix::Pico => -12,
            SiPrefix::Nano => -9,
            SiPrefix::Micro => -6,
            SiPrefix::Milli => -3,
            SiPrefix::One => 0,
            SiPrefix::Kilo => 3,
            SiPrefix::Mega => 6,
            SiPrefix::Giga => 9,
            SiPrefix::Tera => 12,
        }
    }

    /// Display symbol; `µ` is U+00B5. Empty for [`SiPrefix::One`].
    pub fn symbol(self) -> &'static str {
        match self {
            SiPrefix::Femto => "f",
            SiPrefix::Pico => "p",
            SiPrefix::Nano => "n",
            SiPrefix::Micro => "µ",
            SiPrefix::Milli => "m",
            SiPrefix::One => "",
            SiPrefix::Kilo => "k",
            SiPrefix::Mega => "M",
            SiPrefix::Giga => "G",
            SiPrefix::Tera => "T",
        }
    }

    /// Pure-ASCII symbol (`u` for micro).
    pub fn ascii_symbol(self) -> &'static str {
        match self {
            SiPrefix::Micro => "u",
            other => other.symbol(),
        }
    }

    /// The prefixes a picker offers for `unit` (its allowed set, ascending).
    pub fn all_for(unit: &Unit) -> &'static [SiPrefix] {
        unit.allowed_prefixes()
    }

    /// Read one prefix character: `f p n µ μ u m k K M G T`. Case matters
    /// (`m` milli, `M` mega); `K` is accepted as kilo because stored
    /// libraries contain `KΩ`.
    pub fn from_char(c: char) -> Option<SiPrefix> {
        Some(match c {
            'f' => SiPrefix::Femto,
            'p' => SiPrefix::Pico,
            'n' => SiPrefix::Nano,
            'µ' | 'μ' | 'u' => SiPrefix::Micro,
            'm' => SiPrefix::Milli,
            'k' | 'K' => SiPrefix::Kilo,
            'M' => SiPrefix::Mega,
            'G' => SiPrefix::Giga,
            'T' => SiPrefix::Tera,
            _ => return None,
        })
    }
}
