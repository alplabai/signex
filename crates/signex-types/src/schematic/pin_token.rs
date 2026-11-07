//! The one token table for [`PinDirection`], used by the `.snxsym` codec and
//! by the symbol file's JSON form.
//!
//! The first ten tokens are the ones the library's former 10-variant enum
//! wrote, so a file that uses only those types re-saves byte-identical and an
//! older build can still read it. The four newest variants get new tokens.
//! See `crates/signex-types/docs/pin-design.md`, "Stored tokens"`.

use super::PinDirection;

impl PinDirection {
    /// Every variant, in dropdown order.
    pub const ALL: [PinDirection; 14] = [
        Self::Input,
        Self::Bidirectional,
        Self::Output,
        Self::OpenDrainLow,
        Self::Passive,
        Self::ThreeStatable,
        Self::OpenDrainHigh,
        Self::PowerInput,
        Self::DoNotConnect,
        Self::Unclassified,
        Self::PowerOutput,
        Self::GroundReference,
        Self::Differential,
        Self::Clock,
    ];

    /// The text a `.snxsym` file stores for this type.
    #[must_use]
    pub const fn as_token(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Output => "Output",
            Self::Bidirectional => "Bidirectional",
            Self::PowerInput => "Power",
            Self::Passive => "Passive",
            Self::OpenDrainLow => "OpenCollector",
            Self::OpenDrainHigh => "OpenEmitter",
            Self::DoNotConnect => "NotConnected",
            Self::ThreeStatable => "Tristate",
            Self::Unclassified => "Unspecified",
            Self::PowerOutput => "PowerOutput",
            Self::GroundReference => "GroundReference",
            Self::Differential => "Differential",
            Self::Clock => "Clock",
        }
    }

    /// The type a stored token names; `None` for an unknown token.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.as_token() == token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokens the former library enum wrote; each maps to the schematic type
    /// the former `to_lib_symbol` mapping gave it.
    const LEGACY: [(&str, PinDirection); 10] = [
        ("Input", PinDirection::Input),
        ("Output", PinDirection::Output),
        ("Bidirectional", PinDirection::Bidirectional),
        ("Power", PinDirection::PowerInput),
        ("Passive", PinDirection::Passive),
        ("OpenCollector", PinDirection::OpenDrainLow),
        ("OpenEmitter", PinDirection::OpenDrainHigh),
        ("NotConnected", PinDirection::DoNotConnect),
        ("Tristate", PinDirection::ThreeStatable),
        ("Unspecified", PinDirection::Unclassified),
    ];

    #[test]
    fn legacy_tokens_read_to_their_old_meaning_and_write_back_identically() {
        for (token, expected) in LEGACY {
            let read = PinDirection::from_token(token).expect(token);
            assert_eq!(read, expected, "{token}");
            assert_eq!(read.as_token(), token);
        }
    }

    #[test]
    fn every_variant_round_trips_through_its_token() {
        for d in PinDirection::ALL {
            assert_eq!(PinDirection::from_token(d.as_token()), Some(d), "{d:?}");
        }
    }

    #[test]
    fn tokens_are_unique_and_all_covers_every_variant() {
        let mut tokens: Vec<_> = PinDirection::ALL.iter().map(|d| d.as_token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), 14);
        // Exhaustive match: a new variant fails to compile until listed here.
        for d in PinDirection::ALL {
            match d {
                PinDirection::Input
                | PinDirection::Output
                | PinDirection::Bidirectional
                | PinDirection::ThreeStatable
                | PinDirection::Passive
                | PinDirection::PowerInput
                | PinDirection::PowerOutput
                | PinDirection::GroundReference
                | PinDirection::OpenDrainLow
                | PinDirection::OpenDrainHigh
                | PinDirection::Differential
                | PinDirection::Clock
                | PinDirection::DoNotConnect
                | PinDirection::Unclassified => {}
            }
        }
    }

    #[test]
    fn unknown_token_is_none() {
        assert_eq!(PinDirection::from_token("power"), None);
        assert_eq!(PinDirection::from_token(""), None);
    }
}
