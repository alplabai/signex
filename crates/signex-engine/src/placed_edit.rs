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
        SelectedKind::HarnessConnector => sheet.harness.connector(id).map(|c| at(c.position)),
        SelectedKind::HarnessWire => sheet.harness.wire(id).map(|w| at(w.start)),
        // A directive (#2420) is moved on the sheet, not from Properties; a
        // harness entry sits in its connector's slot.
        SelectedKind::SymbolRefField
        | SelectedKind::SymbolValField
        | SelectedKind::Drawing
        | SelectedKind::Directive
        | SelectedKind::HarnessEntry => None,
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
        SelectedKind::HarnessConnector,
        SelectedKind::HarnessWire,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use signex_types::schematic::{Label, LabelType, Symbol};

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
