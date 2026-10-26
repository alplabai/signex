//! Rotate and mirror for a bus entry.
//!
//! The orientation of an entry is the sign pattern of its `size` (see
//! `signex_types::schematic::bus_entry`), and it turns about its bus end, so
//! both operations only rewrite `size`; `position` never moves and the entry
//! stays on its bus.

use signex_types::schematic::{BusEntry, SelectedItem, SelectedKind};

use crate::command::MirrorAxis;

use super::Engine;

impl Engine {
    /// True when the selection is exactly one bus entry present in the document.
    pub fn selection_is_single_bus_entry(&self, items: &[SelectedItem]) -> bool {
        matches!(items, [item] if item.kind == SelectedKind::BusEntry
            && self.document.bus_entries.iter().any(|e| e.uuid == item.uuid))
    }

    pub(super) fn rotate_bus_entry(&mut self, item: &SelectedItem, angle_degrees: f64) -> bool {
        let quarter_turns = (angle_degrees / 90.0).round() as i32;
        if quarter_turns.rem_euclid(4) == 0 {
            return false;
        }
        let Some(entry) = self
            .document
            .bus_entries
            .iter_mut()
            .find(|entry| entry.uuid == item.uuid)
        else {
            return false;
        };
        entry.size = BusEntry::rotated_size(entry.size, quarter_turns);
        true
    }

    pub(super) fn mirror_bus_entry(&mut self, item: &SelectedItem, axis: MirrorAxis) -> bool {
        let Some(entry) = self
            .document
            .bus_entries
            .iter_mut()
            .find(|entry| entry.uuid == item.uuid)
        else {
            return false;
        };
        entry.size = match axis {
            // `Horizontal` is the left-right flip (the `X` key).
            MirrorAxis::Horizontal => BusEntry::flipped_horizontal(entry.size),
            MirrorAxis::Vertical => BusEntry::flipped_vertical(entry.size),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Point, SchematicSheet};
    use uuid::Uuid;

    use super::*;
    use crate::Command;

    fn engine_with_entry(size: (f64, f64)) -> (Engine, SelectedItem) {
        let uuid = Uuid::new_v4();
        let mut sheet = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        sheet.bus_entries.push(BusEntry {
            uuid,
            position: Point::new(10.0, 10.0),
            size,
        });
        (
            Engine::new(sheet).expect("engine"),
            SelectedItem::new(uuid, SelectedKind::BusEntry),
        )
    }

    fn stored(engine: &Engine) -> ((f64, f64), (f64, f64)) {
        let e = &engine.document().bus_entries[0];
        ((e.position.x, e.position.y), e.size)
    }

    #[test]
    fn rotate_turns_the_entry_about_its_bus_end() {
        let (mut engine, item) = engine_with_entry((2.54, 2.54));
        let rotate = |degrees| Command::RotateSelection {
            items: vec![item],
            angle_degrees: degrees,
            keep_wires: false,
        };
        engine.execute(rotate(90.0)).expect("rotate");
        assert_eq!(stored(&engine), ((10.0, 10.0), (2.54, -2.54)));
        engine.execute(rotate(-90.0)).expect("rotate back");
        assert_eq!(stored(&engine), ((10.0, 10.0), (2.54, 2.54)));
        engine.execute(rotate(180.0)).expect("half turn");
        assert_eq!(stored(&engine), ((10.0, 10.0), (-2.54, -2.54)));
    }

    #[test]
    fn mirror_flips_one_axis_and_is_undoable() {
        let (mut engine, item) = engine_with_entry((2.54, 2.54));
        let mirror = |axis| Command::MirrorSelection {
            items: vec![item],
            axis,
            keep_wires: false,
        };
        engine.execute(mirror(MirrorAxis::Horizontal)).expect("x");
        assert_eq!(stored(&engine), ((10.0, 10.0), (-2.54, 2.54)));
        engine.execute(mirror(MirrorAxis::Vertical)).expect("y");
        assert_eq!(stored(&engine), ((10.0, 10.0), (-2.54, -2.54)));
        engine.undo().expect("undo");
        assert_eq!(stored(&engine), ((10.0, 10.0), (-2.54, 2.54)));
    }

    #[test]
    fn single_entry_selection_is_recognised() {
        let (engine, item) = engine_with_entry((2.54, 2.54));
        assert!(engine.selection_is_single_bus_entry(&[item]));
        assert!(!engine.selection_is_single_bus_entry(&[]));
        let ghost = SelectedItem::new(Uuid::new_v4(), SelectedKind::BusEntry);
        assert!(!engine.selection_is_single_bus_entry(&[ghost]));
    }
}
