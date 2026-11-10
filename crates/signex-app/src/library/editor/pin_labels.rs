//! Catalog labels of the pin enums the library editors show.

use signex_i18n::{MessageId, ids};
use signex_library::PinOrientation;
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
}
