//! Absolute placement edits of one placed object (#2418): the Position,
//! Rotation and Size a Properties row shows and a command operand names,
//! turned into the engine command that reaches them. The Properties rows
//! and the `set_object_*` commands both ask here, so a typed value and an
//! agent's operand end in the same edit.

use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};

use crate::Command;

/// The editable geometry of one placed object, in stored mm (Y down from the
/// paper's top). `rotation` is `Some` for a kind whose turn can be set,
/// `size` for a sheet symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGeometry {
    /// The point an absolute move sets: a part's origin, a wire's or bus's
    /// start, a sheet symbol's top-left corner, any other object's position.
    pub position: Point,
    pub rotation: Option<f64>,
    pub size: Option<(f64, f64)>,
}

/// The geometry of `item` on `sheet`, or `None` for a kind with no single
/// editable position (a drawing has its own rows, a field travels with its
/// part) or an id the sheet does not have.
#[must_use]
pub fn placed_geometry(sheet: &SchematicSheet, item: &SelectedItem) -> Option<PlacedGeometry> {
    let id = item.uuid;
    let at = |position| PlacedGeometry {
        position,
        rotation: None,
        size: None,
    };
    match item.kind {
        SelectedKind::Symbol => {
            let s = sheet.symbols.iter().find(|s| s.uuid == id)?;
            Some(PlacedGeometry {
                rotation: Some(s.rotation),
                ..at(s.position)
            })
        }
        SelectedKind::Label => {
            let l = sheet.labels.iter().find(|l| l.uuid == id)?;
            Some(PlacedGeometry {
                rotation: Some(l.rotation),
                ..at(l.position)
            })
        }
        SelectedKind::ChildSheet => {
            let c = sheet.child_sheets.iter().find(|c| c.uuid == id)?;
            Some(PlacedGeometry {
                size: Some(c.size),
                ..at(c.position)
            })
        }
        SelectedKind::Wire => sheet
            .wires
            .iter()
            .find(|w| w.uuid == id)
            .map(|w| at(w.start)),
        SelectedKind::Bus => sheet
            .buses
            .iter()
            .find(|b| b.uuid == id)
            .map(|b| at(b.start)),
        SelectedKind::BusEntry => (sheet.bus_entries.iter())
            .find(|b| b.uuid == id)
            .map(|b| at(b.position)),
        SelectedKind::Junction
        | SelectedKind::NoConnect
        | SelectedKind::NoErcDirective
        | SelectedKind::TextNote
        | SelectedKind::SheetPin => sheet.item_anchor(item).map(at),
        // A directive (#2420) is moved on the sheet, not from Properties.
        SelectedKind::SymbolRefField
        | SelectedKind::SymbolValField
        | SelectedKind::Drawing
        | SelectedKind::Directive => None,
    }
}

/// The placed object `id` names on `sheet`, with its kind: what a command
/// operand's uuid resolves to. `None` when no object with an editable
/// position carries it.
#[must_use]
pub fn placed_item(sheet: &SchematicSheet, id: uuid::Uuid) -> Option<SelectedItem> {
    let kinds = [
        SelectedKind::Symbol,
        SelectedKind::Label,
        SelectedKind::ChildSheet,
        SelectedKind::Wire,
        SelectedKind::Bus,
        SelectedKind::BusEntry,
        SelectedKind::Junction,
        SelectedKind::NoConnect,
        SelectedKind::NoErcDirective,
        SelectedKind::TextNote,
        SelectedKind::SheetPin,
    ];
    kinds
        .into_iter()
        .map(|kind| SelectedItem::new(id, kind))
        .find(|item| placed_geometry(sheet, item).is_some())
}

/// The move that puts `item`'s position at `to`: one `MoveSelection` by the
/// difference, so wires on a part's pins follow it with `keep_wires` as a
/// drag does. A sheet entry stays on its symbol's border, as when dragged.
#[must_use]
pub fn move_to_command(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    to: Point,
    keep_wires: bool,
) -> Option<Command> {
    let from = placed_geometry(sheet, item)?.position;
    Some(Command::MoveSelection {
        items: vec![*item],
        dx: to.x - from.x,
        dy: to.y - from.y,
        keep_wires,
        wiring: signex_types::corner::DragWiringMode::Auto,
    })
}

/// The edit that turns `item` to `degrees` (stored, counter-clockwise): a
/// part turns in place with its wires following (`keep_wires`); a power
/// port and a label take the absolute angle their Rotation
/// rows always set. `None` for a kind without a rotation.
#[must_use]
pub fn rotate_to_command(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    degrees: f64,
    keep_wires: bool,
) -> Option<Command> {
    let degrees = degrees.rem_euclid(360.0);
    match item.kind {
        SelectedKind::Symbol => {
            let s = sheet.symbols.iter().find(|s| s.uuid == item.uuid)?;
            let stored_delta = (degrees - s.rotation).rem_euclid(360.0);
            // Already there (within float noise): the absolute set is a no-op.
            let at_target = !(1e-6..=360.0 - 1e-6).contains(&stored_delta);
            if s.is_power || at_target {
                return Some(Command::SetSymbolRotation {
                    symbol_id: s.uuid,
                    rotation_degrees: if at_target { s.rotation } else { degrees },
                });
            }
            // A turn is counter-clockwise on screen; with one mirror axis set
            // the stored angle runs the other way (`rotate_selected_item`).
            let angle_degrees = if s.mirror_x != s.mirror_y {
                -stored_delta
            } else {
                stored_delta
            };
            Some(Command::RotateSelection {
                items: vec![*item],
                angle_degrees,
                keep_wires,
            })
        }
        SelectedKind::Label => {
            sheet.labels.iter().find(|l| l.uuid == item.uuid)?;
            Some(Command::UpdateLabelProps {
                label_id: item.uuid,
                font_size_mm: None,
                justify: None,
                rotation_degrees: Some(degrees),
            })
        }
        _ => None,
    }
}

/// The start and end of the wire or bus `item` on `sheet`; `None` for any
/// other kind or an id the sheet does not have.
#[must_use]
pub fn run_ends(sheet: &SchematicSheet, item: &SelectedItem) -> Option<(Point, Point)> {
    match item.kind {
        SelectedKind::Wire => (sheet.wires.iter())
            .find(|w| w.uuid == item.uuid)
            .map(|w| (w.start, w.end)),
        SelectedKind::Bus => (sheet.buses.iter())
            .find(|b| b.uuid == item.uuid)
            .map(|b| (b.start, b.end)),
        _ => None,
    }
}

/// The edit that puts one end of the wire or bus `item` (its start when
/// `start`) at `to`, the other end kept: a typed Start or End. `None` for
/// any other kind or an id the sheet does not have.
#[must_use]
pub fn run_end_command(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    start: bool,
    to: Point,
) -> Option<Command> {
    run_ends(sheet, item)?;
    Some(Command::SetRunEnd {
        run: *item,
        start,
        to,
    })
}

/// The edit that sets `props` on the text note `item`; `None` for any other
/// kind or an id the sheet does not have.
#[must_use]
pub fn text_note_props_command(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    props: crate::TextNoteProps,
) -> Option<Command> {
    let is_note =
        item.kind == SelectedKind::TextNote && sheet.text_notes.iter().any(|n| n.uuid == item.uuid);
    is_note.then_some(Command::UpdateTextNoteProps {
        note_id: item.uuid,
        props,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use signex_types::schematic::{Label, LabelType, Symbol, Wire};

    fn wire(start: Point, end: Point) -> Wire {
        Wire {
            uuid: uuid::Uuid::now_v7(),
            start,
            end,
            ..Default::default()
        }
    }

    fn run_item(w: &Wire) -> SelectedItem {
        SelectedItem::new(w.uuid, SelectedKind::Wire)
    }

    /// A typed start moves only the start, a straight wire may turn, and
    /// the edit is one undo step.
    #[test]
    fn a_typed_start_stretches_only_the_start() {
        let w = wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let item = run_item(&w);
        let mut engine = Engine::new(SchematicSheet {
            wires: vec![w],
            ..Default::default()
        })
        .expect("engine");
        let cmd = run_end_command(engine.document(), &item, true, Point::new(2.54, 5.08))
            .expect("a wire");
        assert!(engine.execute(cmd).expect("ran").changed);
        let ends = run_ends(engine.document(), &item).expect("still there");
        assert_eq!(ends, (Point::new(2.54, 5.08), Point::new(10.0, 0.0)));
        engine.undo().expect("undo");
        let ends = run_ends(engine.document(), &item).expect("back");
        assert_eq!(ends, (Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        assert!(!engine.can_undo(), "one undo step");
    }

    /// A typed end stretches only the end; the wire ending on it stays.
    #[test]
    fn a_typed_end_leaves_what_is_attached() {
        let w = wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let other = wire(Point::new(10.0, 0.0), Point::new(10.0, 10.0));
        let item = run_item(&w);
        let mut engine = Engine::new(SchematicSheet {
            wires: vec![w, other.clone()],
            ..Default::default()
        })
        .expect("engine");
        let cmd = run_end_command(engine.document(), &item, false, Point::new(20.0, 5.0))
            .expect("a wire");
        engine.execute(cmd).expect("ran");
        let ends = run_ends(engine.document(), &item).expect("still there");
        assert_eq!(ends, (Point::new(0.0, 0.0), Point::new(20.0, 5.0)));
        let kept = run_ends(engine.document(), &run_item(&other)).expect("kept");
        assert_eq!(kept, (other.start, other.end));
    }

    /// An end typed onto the middle of another wire makes the T a placed
    /// wire would: a junction dot there.
    #[test]
    fn a_typed_end_on_a_wire_gets_the_junction_placement_gets() {
        let trunk = wire(Point::new(0.0, 10.0), Point::new(20.0, 10.0));
        let w = wire(Point::new(10.0, 0.0), Point::new(10.0, 5.0));
        let item = run_item(&w);
        let mut engine = Engine::new(SchematicSheet {
            wires: vec![trunk, w],
            ..Default::default()
        })
        .expect("engine");
        let t = Point::new(10.0, 10.0);
        let cmd = run_end_command(engine.document(), &item, false, t).expect("a wire");
        engine.execute(cmd).expect("ran");
        let key = signex_net::pt_key(&t);
        assert!(
            (engine.document().junctions.iter()).any(|j| signex_net::pt_key(&j.position) == key),
            "a junction at the T"
        );
    }

    #[test]
    fn a_bus_end_is_set_and_an_end_on_the_other_changes_nothing() {
        let bus = signex_types::schematic::Bus {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        };
        let item = SelectedItem::new(bus.uuid, SelectedKind::Bus);
        let mut engine = Engine::new(SchematicSheet {
            buses: vec![bus],
            ..Default::default()
        })
        .expect("engine");
        let onto_start =
            run_end_command(engine.document(), &item, false, Point::new(0.0, 0.0)).expect("a bus");
        assert!(!engine.execute(onto_start).expect("ran").changed);
        let cmd =
            run_end_command(engine.document(), &item, false, Point::new(10.0, 7.5)).expect("a bus");
        assert!(engine.execute(cmd).expect("ran").changed);
        let ends = run_ends(engine.document(), &item).expect("bus");
        assert_eq!(ends.1, Point::new(10.0, 7.5));
        let junction = SelectedItem::new(item.uuid, SelectedKind::Junction);
        assert!(run_end_command(engine.document(), &junction, true, ends.0).is_none());
    }

    #[test]
    fn a_locked_wire_refuses_a_typed_end() {
        let mut w = wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        w.locked = true;
        let item = run_item(&w);
        let engine = Engine::new(SchematicSheet {
            wires: vec![w],
            ..Default::default()
        })
        .expect("engine");
        let cmd =
            run_end_command(engine.document(), &item, true, Point::new(1.0, 1.0)).expect("a wire");
        assert!(engine.lock_refusal(&cmd).is_some());
    }

    fn part(rotation: f64, mirror_x: bool) -> Symbol {
        Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            position: Point::new(10.0, 10.0),
            rotation,
            mirror_x,
            ..Default::default()
        }
    }

    #[test]
    fn a_part_moves_by_the_difference_and_a_wire_by_its_start() {
        let s = part(0.0, false);
        let item = SelectedItem::new(s.uuid, SelectedKind::Symbol);
        let sheet = SchematicSheet {
            symbols: vec![s],
            ..Default::default()
        };
        let cmd = move_to_command(&sheet, &item, Point::new(12.54, 7.46), true);
        assert!(matches!(
            cmd,
            Some(Command::MoveSelection { dx, dy, keep_wires: true, .. })
                if (dx - 2.54).abs() < 1e-9 && (dy + 2.54).abs() < 1e-9
        ));
        let missing = SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Wire);
        assert!(move_to_command(&sheet, &missing, Point::new(0.0, 0.0), false).is_none());
    }

    /// Every rotation lands on the asked angle, mirrored or not.
    #[test]
    fn a_part_turns_to_the_absolute_angle_asked() {
        for mirror_x in [false, true] {
            for (from, to) in [(0.0, 90.0), (90.0, 0.0), (270.0, 180.0), (180.0, 180.0)] {
                let s = part(from, mirror_x);
                let item = SelectedItem::new(s.uuid, SelectedKind::Symbol);
                let mut engine = Engine::new(SchematicSheet {
                    symbols: vec![s],
                    ..Default::default()
                })
                .expect("engine");
                let cmd = rotate_to_command(engine.document(), &item, to, false).expect("a turn");
                engine.execute(cmd).expect("turned");
                let now = engine.document().symbols[0].rotation;
                let off = (now - to).rem_euclid(360.0);
                assert!(
                    !(1e-6..=360.0 - 1e-6).contains(&off),
                    "mirror {mirror_x}: {from} -> {to} ended at {now}"
                );
            }
        }
    }

    #[test]
    fn a_label_takes_the_absolute_angle_and_a_junction_has_none() {
        let label = Label::new("NET", Point::new(0.0, 0.0), LabelType::Net);
        let item = SelectedItem::new(label.uuid, SelectedKind::Label);
        let sheet = SchematicSheet {
            labels: vec![label],
            ..Default::default()
        };
        assert!(matches!(
            rotate_to_command(&sheet, &item, -90.0, false),
            Some(Command::UpdateLabelProps { rotation_degrees: Some(r), .. }) if r == 270.0
        ));
        let junction = SelectedItem::new(item.uuid, SelectedKind::Junction);
        assert!(rotate_to_command(&sheet, &junction, 90.0, false).is_none());
    }
}
