//! Rotate and mirror for a bus entry.
//!
//! The orientation of an entry is the sign pattern of its `size` (see
//! `signex_types::schematic::bus_entry`), and it turns about its bus end, so
//! both operations only rewrite `size`; `position` never moves and the entry
//! stays on its bus.

use signex_net::{point_on_segment, pt_key};
use signex_types::schematic::{BusEntry, Point, SelectedItem, SelectedKind};

use crate::command::MirrorAxis;

use super::Engine;

/// Entries and entry far ends a bus drag carries (see `bus_followers`).
#[derive(Default)]
pub(crate) struct BusFollowers {
    pub entries: Vec<(SelectedItem, f64, f64)>,
    pub far_ends: Vec<((i64, i64), f64, f64)>,
}

impl Engine {
    /// True when the selection is exactly one bus entry present in the document.
    pub fn selection_is_single_bus_entry(&self, items: &[SelectedItem]) -> bool {
        matches!(items, [item] if item.kind == SelectedKind::BusEntry
            && self.document.bus_entries.iter().any(|e| e.uuid == item.uuid))
    }

    /// What a drag of a bus carries along: every entry sitting on a moved bus
    /// (its bus end on the bus segment before the move) that is not itself
    /// moving, with the bus delta, plus - when `keep_wires` - the far
    /// ends of those entries, so the wires attached there can follow.
    pub(crate) fn bus_followers(
        &self,
        moves: &[(SelectedItem, f64, f64)],
        keep_wires: bool,
    ) -> BusFollowers {
        let mut out = BusFollowers::default();
        for (item, dx, dy) in moves.iter().filter(|(i, ..)| i.kind == SelectedKind::Bus) {
            let Some(bus) = self.document.buses.iter().find(|b| b.uuid == item.uuid) else {
                continue;
            };
            let (a, b) = (pt_key(&bus.start), pt_key(&bus.end));
            for entry in &self.document.bus_entries {
                let already = moves.iter().any(|(i, ..)| i.uuid == entry.uuid)
                    || out.entries.iter().any(|(i, ..)| i.uuid == entry.uuid);
                if already || !point_on_segment(pt_key(&entry.position), a, b) {
                    continue;
                }
                out.entries.push((
                    SelectedItem::new(entry.uuid, SelectedKind::BusEntry),
                    *dx,
                    *dy,
                ));
                if keep_wires {
                    let far = Point::new(
                        entry.position.x + entry.size.0,
                        entry.position.y + entry.size.1,
                    );
                    out.far_ends.push((pt_key(&far), *dx, *dy));
                }
            }
        }
        out
    }

    /// Shift the endpoints of wires that ended on a carried entry far end.
    /// Wires that were themselves moved are left alone. Returns the wires hit.
    pub(crate) fn follow_bus_entry_ends(
        &mut self,
        followers: &BusFollowers,
        moved: &[SelectedItem],
    ) -> Vec<SelectedItem> {
        let mut touched = Vec::new();
        for wire in self
            .document
            .wires
            .iter_mut()
            .filter(|w| !moved.iter().any(|m| m.uuid == w.uuid))
        {
            let mut hit = false;
            for (key, dx, dy) in &followers.far_ends {
                if pt_key(&wire.start) == *key {
                    wire.start.x += dx;
                    wire.start.y += dy;
                    hit = true;
                } else if pt_key(&wire.end) == *key {
                    wire.end.x += dx;
                    wire.end.y += dy;
                    hit = true;
                }
            }
            if hit {
                touched.push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
            }
        }
        touched
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
        let uuid = Uuid::now_v7();
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
        let ghost = SelectedItem::new(Uuid::now_v7(), SelectedKind::BusEntry);
        assert!(!engine.selection_is_single_bus_entry(&[ghost]));
    }
}

#[cfg(test)]
mod follow_tests {
    use signex_types::schematic::{Bus, Point, SchematicSheet, Wire};
    use uuid::Uuid;

    use super::*;
    use crate::Command;

    fn sheet() -> (Engine, SelectedItem, SelectedItem) {
        let (bus_id, entry_id) = (Uuid::now_v7(), Uuid::now_v7());
        let mut s = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        s.buses.push(Bus {
            uuid: bus_id,
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 20.0),
        });
        s.bus_entries.push(BusEntry {
            uuid: entry_id,
            position: Point::new(0.0, 10.0),
            size: (2.54, 2.54),
        });
        s.bus_entries.push(BusEntry {
            uuid: Uuid::now_v7(),
            position: Point::new(50.0, 10.0),
            size: (2.54, 2.54),
        });
        s.wires.push(Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(2.54, 12.54),
            end: Point::new(12.54, 12.54),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
        });
        (
            Engine::new(s).expect("engine"),
            SelectedItem::new(bus_id, SelectedKind::Bus),
            SelectedItem::new(entry_id, SelectedKind::BusEntry),
        )
    }

    fn drag(engine: &mut Engine, bus: SelectedItem, keep_wires: bool) {
        engine
            .execute(Command::MoveSelection {
                items: vec![bus],
                dx: 5.0,
                dy: 0.0,
                keep_wires,
            })
            .expect("move");
    }

    #[test]
    fn dragging_a_bus_carries_its_entries_and_wire_ends_in_one_step() {
        let (mut engine, bus, _) = sheet();
        let before = engine.document().clone();
        drag(&mut engine, bus, true);
        let d = engine.document();
        assert_eq!(d.buses[0].start.x, 5.0);
        assert_eq!(d.bus_entries[0].position, Point::new(5.0, 10.0));
        assert_eq!(d.bus_entries[0].size, (2.54, 2.54));
        assert_eq!(
            d.bus_entries[1].position,
            Point::new(50.0, 10.0),
            "off-bus entry stays"
        );
        assert_eq!(d.wires[0].start, Point::new(7.54, 12.54));
        assert_eq!(d.wires[0].end, Point::new(12.54, 12.54), "far end stays");
        engine.undo().expect("one undo");
        assert_eq!(
            engine.document().bus_entries[0].position,
            before.bus_entries[0].position
        );
        assert_eq!(engine.document().wires[0].start, before.wires[0].start);
        assert_eq!(engine.document().buses[0].start, before.buses[0].start);
    }

    #[test]
    fn plain_drag_leaves_wires_but_still_carries_entries() {
        let (mut engine, bus, _) = sheet();
        drag(&mut engine, bus, false);
        let d = engine.document();
        assert_eq!(d.bus_entries[0].position, Point::new(5.0, 10.0));
        assert_eq!(d.wires[0].start, Point::new(2.54, 12.54));
    }

    /// A bus with an entry whose far end is wired to pin 1 of R1; the bus and
    /// R1 are selected together and moved. The wire's pin end must still be
    /// carried by the rubber band, so pin 1 stays connected.
    #[test]
    fn bus_and_symbol_moved_together_keep_the_entry_wire_on_the_pin() {
        use crate::rubber_band_tests::{add_resistor, add_wire, net_shape, symbol_item};
        let mut sheet = SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        let bus_id = Uuid::now_v7();
        sheet.buses.push(Bus {
            uuid: bus_id,
            start: Point::new(30.0, 40.0),
            end: Point::new(30.0, 60.0),
        });
        sheet.bus_entries.push(BusEntry {
            uuid: Uuid::now_v7(),
            position: Point::new(30.0, 47.46),
            size: (2.54, 2.54),
        });
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        add_wire(&mut sheet, (32.54, 50.0), (45.0, 50.0));
        let mut engine = Engine::new(sheet).expect("engine");
        let nets = net_shape(engine.document());

        engine
            .execute(Command::MoveSelection {
                items: vec![
                    SelectedItem::new(bus_id, SelectedKind::Bus),
                    symbol_item(r1),
                ],
                dx: 5.0,
                dy: 0.0,
                keep_wires: true,
            })
            .expect("move");

        let d = engine.document();
        assert_eq!(d.bus_entries[0].position, Point::new(35.0, 47.46));
        let w = &d.wires[0];
        let ends = [(w.start.x, w.start.y), (w.end.x, w.end.y)];
        assert!(ends.contains(&(37.54, 50.0)), "entry end follows: {ends:?}");
        assert!(ends.contains(&(50.0, 50.0)), "pin 1 end follows: {ends:?}");
        assert_eq!(net_shape(d), nets, "pin 1 stays connected");
        assert!(engine.take_warnings().is_empty());
    }

    #[test]
    fn an_entry_selected_with_its_bus_moves_once() {
        let (mut engine, bus, entry) = sheet();
        engine
            .execute(Command::MoveSelection {
                items: vec![bus, entry],
                dx: 5.0,
                dy: 0.0,
                keep_wires: false,
            })
            .expect("move");
        assert_eq!(
            engine.document().bus_entries[0].position,
            Point::new(5.0, 10.0)
        );
    }
}
