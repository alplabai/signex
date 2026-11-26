//! Catalog labels of the pin enums the library editors show.

use signex_i18n::{MessageId, ids};
use signex_library::{PinOrientation, PinSymbolKind};
use signex_types::schematic::PinDirection;

/// The label of a pin's electrical type; the `en-US` text is
/// [`PinDirection::label`].
pub(crate) fn direction_message(direction: PinDirection) -> MessageId {
    match direction {
        PinDirection::Input => ids::EDITORS_PIN_TYPE_INPUT,
        PinDirection::Output => ids::EDITORS_PIN_TYPE_OUTPUT,
        PinDirection::Bidirectional => ids::EDITORS_PIN_TYPE_BIDIRECTIONAL,
        PinDirection::ThreeStatable => ids::EDITORS_PIN_TYPE_THREE_STATABLE,
        PinDirection::Passive => ids::EDITORS_PIN_TYPE_PASSIVE,
        PinDirection::PowerInput => ids::EDITORS_PIN_TYPE_POWER_INPUT,
        PinDirection::PowerOutput => ids::EDITORS_PIN_TYPE_POWER_OUTPUT,
        PinDirection::GroundReference => ids::EDITORS_PIN_TYPE_GROUND_REFERENCE,
        PinDirection::OpenDrainLow => ids::EDITORS_PIN_TYPE_OPEN_DRAIN_LOW,
        PinDirection::OpenDrainHigh => ids::EDITORS_PIN_TYPE_OPEN_DRAIN_HIGH,
        PinDirection::Differential => ids::EDITORS_PIN_TYPE_DIFFERENTIAL,
        PinDirection::Clock => ids::EDITORS_PIN_TYPE_CLOCK,
        PinDirection::DoNotConnect => ids::EDITORS_PIN_TYPE_DO_NOT_CONNECT,
        PinDirection::Unclassified => ids::EDITORS_PIN_TYPE_UNCLASSIFIED,
    }
}

/// The label of the direction a pin points from its body.
pub(crate) fn orientation_message(orientation: PinOrientation) -> MessageId {
    match orientation {
        PinOrientation::Up => ids::EDITORS_PIN_ORIENTATION_UP,
        PinOrientation::Down => ids::EDITORS_PIN_ORIENTATION_DOWN,
        PinOrientation::Left => ids::EDITORS_PIN_ORIENTATION_LEFT,
        PinOrientation::Right => ids::EDITORS_PIN_ORIENTATION_RIGHT,
        // `PinOrientation` is non_exhaustive; Right is its default.
        _ => ids::EDITORS_PIN_ORIENTATION_RIGHT,
    }
}

/// The pin direction a pin summary carries by its debug name
/// (`PowerInput`).
pub(crate) fn direction_by_name(debug_name: &str) -> Option<PinDirection> {
    use PinDirection as D;
    Some(match debug_name {
        "Input" => D::Input,
        "Output" => D::Output,
        "Bidirectional" => D::Bidirectional,
        "ThreeStatable" => D::ThreeStatable,
        "Passive" => D::Passive,
        "PowerInput" => D::PowerInput,
        "PowerOutput" => D::PowerOutput,
        "GroundReference" => D::GroundReference,
        "OpenDrainLow" => D::OpenDrainLow,
        "OpenDrainHigh" => D::OpenDrainHigh,
        "Differential" => D::Differential,
        "Clock" => D::Clock,
        "DoNotConnect" => D::DoNotConnect,
        "Unclassified" => D::Unclassified,
        _ => return None,
    })
}

/// The label of a pin direction, by its debug name (`PowerInput`), as the
/// pin summary carries it.
pub(crate) fn electrical_message(debug_name: &str) -> Option<MessageId> {
    direction_by_name(debug_name).map(direction_message)
}

/// The pin orientation a pin summary carries by its debug name (`Right`).
pub(crate) fn orientation_by_name(debug_name: &str) -> Option<PinOrientation> {
    Some(match debug_name {
        "Up" => PinOrientation::Up,
        "Down" => PinOrientation::Down,
        "Left" => PinOrientation::Left,
        "Right" => PinOrientation::Right,
        _ => return None,
    })
}

/// Every pin symbol (the Inside / Inside Edge / Outside Edge / Outside
/// glyphs), in the order the Properties dropdowns list them.
pub(crate) const PIN_SYMBOLS: [PinSymbolKind; 15] = [
    PinSymbolKind::None,
    PinSymbolKind::Dot,
    PinSymbolKind::ClockEdge,
    PinSymbolKind::ActiveLowInput,
    PinSymbolKind::ActiveLowOutput,
    PinSymbolKind::SchmittTrigger,
    PinSymbolKind::Analog,
    PinSymbolKind::Digital,
    PinSymbolKind::ShiftRight,
    PinSymbolKind::ShiftLeft,
    PinSymbolKind::Pi,
    PinSymbolKind::Sigma,
    PinSymbolKind::OpenCollector,
    PinSymbolKind::OpenEmitter,
    PinSymbolKind::HiZ,
];

/// The label of a pin symbol.
pub(crate) fn symbol_kind_message(kind: PinSymbolKind) -> MessageId {
    match kind {
        PinSymbolKind::Dot => ids::EDITORS_PIN_SYMBOL_DOT,
        PinSymbolKind::ClockEdge => ids::EDITORS_PIN_SYMBOL_CLOCK_EDGE,
        PinSymbolKind::ActiveLowInput => ids::EDITORS_PIN_SYMBOL_ACTIVE_LOW_INPUT,
        PinSymbolKind::ActiveLowOutput => ids::EDITORS_PIN_SYMBOL_ACTIVE_LOW_OUTPUT,
        PinSymbolKind::SchmittTrigger => ids::EDITORS_PIN_SYMBOL_SCHMITT_TRIGGER,
        PinSymbolKind::Analog => ids::EDITORS_PIN_SYMBOL_ANALOG,
        PinSymbolKind::Digital => ids::EDITORS_PIN_SYMBOL_DIGITAL,
        PinSymbolKind::ShiftRight => ids::EDITORS_PIN_SYMBOL_SHIFT_RIGHT,
        PinSymbolKind::ShiftLeft => ids::EDITORS_PIN_SYMBOL_SHIFT_LEFT,
        PinSymbolKind::Pi => ids::EDITORS_PIN_SYMBOL_PI,
        PinSymbolKind::Sigma => ids::EDITORS_PIN_SYMBOL_SIGMA,
        PinSymbolKind::OpenCollector => ids::EDITORS_PIN_SYMBOL_OPEN_COLLECTOR,
        PinSymbolKind::OpenEmitter => ids::EDITORS_PIN_SYMBOL_OPEN_EMITTER,
        PinSymbolKind::HiZ => ids::EDITORS_PIN_SYMBOL_HI_Z,
        // `PinSymbolKind` is non_exhaustive; None is its default.
        _ => ids::EDITORS_PIN_SYMBOL_NONE,
    }
}

/// A part's letter: `A` for part 1 up to `Z` for part 26, `*` for the
/// shared part 0, the number past 26.
pub(crate) fn part_label(part_number: u8) -> String {
    match part_number {
        0 => "*".to_string(),
        n @ 1..=26 => char::from(b'A' + n - 1).to_string(),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECTIONS: [PinDirection; 14] = [
        PinDirection::Input,
        PinDirection::Output,
        PinDirection::Bidirectional,
        PinDirection::ThreeStatable,
        PinDirection::Passive,
        PinDirection::PowerInput,
        PinDirection::PowerOutput,
        PinDirection::GroundReference,
        PinDirection::OpenDrainLow,
        PinDirection::OpenDrainHigh,
        PinDirection::Differential,
        PinDirection::Clock,
        PinDirection::DoNotConnect,
        PinDirection::Unclassified,
    ];

    #[test]
    fn the_en_us_direction_labels_are_the_domain_labels() {
        let en = signex_i18n::I18n::en_us();
        for d in DIRECTIONS {
            let shown = signex_i18n::t!(en, direction_message(d)).into_string();
            assert_eq!(shown, d.label(), "{d:?}");
        }
    }

    // Moved with the Pins panel's labels (mockup row 122).
    #[test]
    fn every_pin_direction_has_a_catalog_label() {
        for d in DIRECTIONS {
            assert!(electrical_message(&format!("{d:?}")).is_some(), "{d:?}");
        }
        assert_eq!(electrical_message("NotAVariant"), None);
    }

    #[test]
    fn every_pin_orientation_has_a_catalog_label() {
        use signex_library::PinOrientation as O;
        for o in [O::Up, O::Down, O::Left, O::Right] {
            assert!(
                orientation_by_name(&format!("{o:?}"))
                    .map(orientation_message)
                    .is_some(),
                "{o:?}"
            );
        }
    }

    #[test]
    fn part_label_names_parts_by_letter() {
        assert_eq!(part_label(1), "A");
        assert_eq!(part_label(2), "B");
        assert_eq!(part_label(0), "*");
        assert_eq!(part_label(27), "27");
    }

    #[test]
    fn every_pin_symbol_has_its_own_catalog_label() {
        let labels: std::collections::HashSet<_> = PIN_SYMBOLS
            .iter()
            .map(|k| symbol_kind_message(*k))
            .collect();
        assert_eq!(labels.len(), PIN_SYMBOLS.len());
    }
}
