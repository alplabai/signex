//! The one component a board command acts on (`swap_pins`, `swap_gates`,
//! `place_board_reuse_block`, #2326): named by `ids`, its uuid, or by
//! `designator`, exactly one of the two. The board menu names it by uuid, so
//! it acts on the component clicked even where a designator is shared.

use signex_i18n::{Translatable, ids};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{CommandArgs, OperandValue};

/// The component `args` names on `board`: `Some(uuid)` by `ids`, `None` by
/// `designator`, which the caller looks up as it always has. Refused when
/// both or neither are given, when `ids` names more than one item, or when
/// its uuid is no footprint of `board`. A blank `designator` is not given.
pub(crate) fn component_by_uuid(
    board: &PcbBoard,
    args: &CommandArgs,
) -> Result<Option<Uuid>, Translatable> {
    let by_designator = matches!(
        args.operand("designator"),
        Some(OperandValue::Text(text)) if !text.trim().is_empty()
    );
    match (args.targets("ids"), by_designator) {
        (Some(_), true) => Err(Translatable::new(ids::BOARD_COMPONENT_BOTH)),
        (None, false) => Err(Translatable::new(ids::BOARD_COMPONENT_NONE)),
        (None, true) => Ok(None),
        (Some([id]), false) if board.footprints.iter().any(|fp| fp.uuid == *id) => Ok(Some(*id)),
        (Some([id]), false) => {
            Err(Translatable::new(ids::BOARD_TARGET_UNKNOWN).arg("id", id.to_string()))
        }
        (Some(many), false) => {
            Err(Translatable::new(ids::BOARD_COMPONENT_MANY).arg("count", many.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_i18n::RenderTranslatable;

    use super::*;
    use crate::keymap::AppCommandId;

    const FP: Uuid = Uuid::from_u128(0xf0);

    fn board() -> PcbBoard {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::from_u128(1),
            "footprints": [{
                "uuid": FP, "reference": "U1", "value": "X", "footprint_id": "SO8",
                "position": {"x": 0.0, "y": 0.0}, "rotation": 0.0, "layer": "TopCopper",
                "pads": []
            }]
        }))
        .unwrap()
    }

    fn named(operands: &[&str]) -> Result<Option<Uuid>, String> {
        let command = AppCommandId::new("swap_pins").unwrap();
        // `/b.snxpcb` is not absolute on Windows (no drive); the file is never read.
        let document = format!(
            "document={}",
            std::env::temp_dir().join("b.snxpcb").display()
        );
        let mut texts = vec![document.as_str(), "pins=1,2"];
        texts.extend_from_slice(operands);
        let args = CommandArgs::parse_operands(&command, &texts).expect("operands parse");
        component_by_uuid(&board(), &args).map_err(|reason| reason.render_en_us())
    }

    #[test]
    fn ids_names_the_footprint_and_designator_leaves_it_to_the_caller() {
        assert_eq!(named(&[&format!("ids={FP}")]), Ok(Some(FP)));
        assert_eq!(named(&["designator=U1"]), Ok(None));
    }

    #[test]
    fn both_neither_many_or_an_unknown_uuid_are_refused() {
        let both = named(&[&format!("ids={FP}"), "designator=U1"]).unwrap_err();
        assert!(both.contains("not both"), "{both}");
        let neither = named(&[]).unwrap_err();
        assert!(
            neither.contains("by ids (its uuid) or by designator"),
            "{neither}"
        );
        let blank = named(&["designator= "]).unwrap_err();
        assert_eq!(blank, neither, "a blank designator is not given");
        let other = Uuid::from_u128(0xf1);
        let many = named(&[&format!("ids={FP},{other}")]).unwrap_err();
        assert!(many.contains("names 2 items"), "{many}");
        let unknown = named(&[&format!("ids={other}")]).unwrap_err();
        assert!(
            unknown.contains(&other.to_string()) && unknown.contains("names nothing"),
            "{unknown}"
        );
    }
}
