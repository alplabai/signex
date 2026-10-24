//! The ghost shown while a selection is dragged or floated (click-click
//! Move): the selection shifted by the cursor delta and, when "Keep wires
//! connected when moving" is on, the attached wires rubber-banded exactly as
//! the engine will commit them (#963).

use crate::schematic_runtime::SchematicRenderSnapshot;
use signex_types::schematic::SelectedItem;

pub(super) fn snapshot(
    snap: &SchematicRenderSnapshot,
    selection: &[SelectedItem],
    dx: f64,
    dy: f64,
    keep_wires: bool,
) -> SchematicRenderSnapshot {
    let mut shifted = super::shift_snapshot_for_selection(snap, selection, dx, dy);
    if keep_wires {
        // Tips come from the unshifted sheet; the follow pass then edits
        // the shifted copy, whose unselected wires are still in place.
        let tips = signex_engine::moved_pin_tips(snap, selection);
        signex_engine::follow_moved_pins(&mut shifted, &tips, selection, dx, dy);
    }
    shifted
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedKind,
        Symbol, Wire,
    };

    fn sheet_with_wired_resistor() -> (SchematicSheet, SelectedItem) {
        let mut sheet = SchematicSheet::default();
        let pin = LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(-5.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        };
        sheet.lib_symbols.insert(
            "R".to_string(),
            LibSymbol {
                id: "R".to_string(),
                reference: String::new(),
                value: String::new(),
                footprint: String::new(),
                datasheet: String::new(),
                description: String::new(),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins: vec![pin],
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
        let uuid = uuid::Uuid::new_v4();
        sheet.symbols.push(Symbol {
            uuid,
            lib_id: "R".to_string(),
            position: Point::new(50.0, 50.0),
            ..Default::default()
        });
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::new_v4(),
            start: Point::new(30.0, 50.0),
            end: Point::new(45.0, 50.0),
            ..Default::default()
        });
        (sheet, SelectedItem::new(uuid, SelectedKind::Symbol))
    }

    #[test]
    fn the_ghost_rubber_bands_the_wires_when_the_preference_is_on() {
        let (sheet, item) = sheet_with_wired_resistor();
        let ghost = snapshot(&sheet, &[item], 0.0, 10.0, true);
        assert_eq!(ghost.symbols[0].position, Point::new(50.0, 60.0));
        assert_eq!(ghost.wires.len(), 2, "a bend wire reaches the new pin tip");
        assert!(
            ghost
                .wires
                .iter()
                .any(|w| w.end == Point::new(45.0, 60.0) || w.start == Point::new(45.0, 60.0)),
            "{:?}",
            ghost.wires
        );
        assert_eq!(sheet.wires.len(), 1, "the source snapshot is untouched");
    }

    #[test]
    fn the_ghost_leaves_wires_alone_when_the_preference_is_off() {
        let (sheet, item) = sheet_with_wired_resistor();
        let ghost = snapshot(&sheet, &[item], 0.0, 10.0, false);
        assert_eq!(ghost.symbols[0].position, Point::new(50.0, 60.0));
        assert_eq!(ghost.wires.len(), 1);
        assert_eq!(ghost.wires[0].end, Point::new(45.0, 50.0));
    }
}
