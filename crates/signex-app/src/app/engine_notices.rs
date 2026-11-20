//! The engine's typed notices (`signex_engine::EngineNotice`) worded through
//! the catalog for the Messages panel (ADR-0006: the core says what
//! happened, the view words it).

use signex_engine::{EngineNotice, RefusedPinDot};
use signex_i18n::{Arg, I18n, MessageId, ids};

/// How many names a line lists before "and N more".
const NAMED: usize = 3;

/// The Messages line of `notice`.
pub(crate) fn notice_line(i18n: &I18n, notice: &EngineNotice) -> String {
    match notice {
        EngineNotice::DraggedWireLeft { held } => names_line(
            i18n,
            held,
            ids::PANELS_MESSAGES_DRAGGED_WIRE_LEFT_ONE,
            ids::PANELS_MESSAGES_DRAGGED_WIRE_LEFT_MANY,
        ),
        EngineNotice::GripEndLeft { at } => i18n.message_plain(
            ids::PANELS_MESSAGES_GRIP_END_LEFT,
            // Lengths go as text from our own formatter, never a locale number.
            &[
                ("x", Arg::from(at.x.to_string())),
                ("y", Arg::from(at.y.to_string())),
            ],
        ),
        EngineNotice::HeldPinsNotJoined { pins } => names_line(
            i18n,
            pins,
            ids::PANELS_MESSAGES_HELD_PINS_ONE,
            ids::PANELS_MESSAGES_HELD_PINS_MANY,
        ),
        EngineNotice::PinDotRefused { pins } => refused_line(i18n, pins),
    }
}

/// `one` naming the single entry of `names`, else `many` with the count and
/// the first few.
fn names_line(i18n: &I18n, names: &[String], one: MessageId, many: MessageId) -> String {
    if let [name] = names {
        return i18n.message_plain(one, &[("name", Arg::from(name.as_str()))]);
    }
    i18n.message_plain(
        many,
        &[
            ("count", Arg::from(names.len())),
            ("names", Arg::from(shown(i18n, names))),
        ],
    )
}

/// The first few of `names`, then "and N more" when there are others.
fn shown(i18n: &I18n, names: &[String]) -> String {
    let first = names
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() <= NAMED {
        return first;
    }
    i18n.message_plain(
        ids::PANELS_MESSAGES_LIST_MORE,
        &[
            ("names", Arg::from(first)),
            ("count", Arg::from(names.len() - NAMED)),
        ],
    )
}

/// The line for pin ends left without a dot that would merge two nets.
fn refused_line(i18n: &I18n, pins: &[RefusedPinDot]) -> String {
    let net = |name: &Option<String>| {
        name.clone()
            .unwrap_or_else(|| i18n.message_plain(ids::PANELS_MESSAGES_UNNAMED_NET, &[]))
    };
    let args = |r: &RefusedPinDot| {
        [
            ("pin", Arg::from(r.pin.as_str())),
            ("through", Arg::from(net(&r.through))),
            ("other", Arg::from(net(&r.other))),
        ]
    };
    if let [only] = pins {
        return i18n.message_plain(ids::PANELS_MESSAGES_PIN_DOT_REFUSED_ONE, &args(only));
    }
    let entries: Vec<String> = (pins.iter())
        .map(|r| i18n.message_plain(ids::PANELS_MESSAGES_PIN_DOT_REFUSED_ENTRY, &args(r)))
        .collect();
    i18n.message_plain(
        ids::PANELS_MESSAGES_PIN_DOT_REFUSED_MANY,
        &[
            ("count", Arg::from(pins.len())),
            ("names", Arg::from(shown(i18n, &entries))),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;

    fn en() -> &'static I18n {
        I18n::en_us_ref()
    }

    #[test]
    fn each_notice_reads_as_its_catalog_line() {
        let one = EngineNotice::DraggedWireLeft {
            held: vec!["R7 pin 1".to_string()],
        };
        assert_eq!(
            notice_line(en(), &one),
            "could not keep R7 pin 1 on the dragged wire"
        );
        let five = EngineNotice::HeldPinsNotJoined {
            pins: (1..=5).map(|n| format!("R{n} pin 1")).collect(),
        };
        assert_eq!(
            notice_line(en(), &five),
            "5 pins kept their nets and were not joined to the wires they end on: \
             R1 pin 1, R2 pin 1, R3 pin 1 and 2 more"
        );
        let grip = EngineNotice::GripEndLeft {
            at: Point::new(10.0, 20.5),
        };
        assert_eq!(
            notice_line(en(), &grip),
            "could not keep the wires at (10, 20.5) on the dragged end"
        );
        let refused = EngineNotice::PinDotRefused {
            pins: vec![RefusedPinDot {
                pin: "TP1 pin 1".to_string(),
                through: Some("A".to_string()),
                other: None,
            }],
        };
        assert_eq!(
            notice_line(en(), &refused),
            "could not join TP1 pin 1 to the wires it ends on: they carry two nets, \
             A and an unnamed net"
        );
    }
}
