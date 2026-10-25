//! The ghost shown while a selection is dragged or floated (click-click
//! Move): the selection shifted by the cursor delta and, when "Keep wires
//! connected when moving" is on, the attached wires rubber-banded exactly as
//! the engine will commit them (#963).

use crate::schematic_runtime::SchematicRenderSnapshot;
use signex_types::schematic::SelectedItem;

/// The drag delta after grid snapping: the PRIMARY selected item's
/// connection point (its stored `position`) lands on a grid dot after the
/// move, not just the delta. Snapping only the delta would preserve an
/// off-grid origin; users expect the endpoint to be on-grid. The commit and
/// the drag ghost both call this, so the preview shows exactly the
/// topology the drop will produce.
pub(crate) fn snapped_move_delta(
    snap: Option<&SchematicRenderSnapshot>,
    selected: &[SelectedItem],
    dx: f64,
    dy: f64,
    snap_enabled: bool,
    grid_mm: f64,
) -> (f64, f64) {
    if !snap_enabled {
        return (dx, dy);
    }
    let primary = selected
        .first()
        .and_then(|item| snap.and_then(|snap| primary_anchor_world(snap, item)));
    match primary {
        Some((px, py)) => {
            let snapped_x = ((px + dx) / grid_mm).round() * grid_mm;
            let snapped_y = ((py + dy) / grid_mm).round() * grid_mm;
            (snapped_x - px, snapped_y - py)
        }
        None => (
            (dx / grid_mm).round() * grid_mm,
            (dy / grid_mm).round() * grid_mm,
        ),
    }
}

/// Resolve a selected item's primary anchor — the world point that should
/// snap to the grid (connection point for labels/wires/symbols, etc.).
fn primary_anchor_world(
    snap: &crate::schematic_runtime::SchematicRenderSnapshot,
    item: &signex_types::schematic::SelectedItem,
) -> Option<(f64, f64)> {
    use signex_types::schematic::SelectedKind;
    match item.kind {
        SelectedKind::Label => snap
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(|l| (l.position.x, l.position.y)),
        SelectedKind::Symbol => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .map(|s| (s.position.x, s.position.y)),
        SelectedKind::Wire => snap
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(|w| (w.start.x, w.start.y)),
        SelectedKind::Bus => snap
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(|b| (b.start.x, b.start.y)),
        SelectedKind::Junction => snap
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(|j| (j.position.x, j.position.y)),
        SelectedKind::NoConnect => snap
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(|n| (n.position.x, n.position.y)),
        SelectedKind::NoErcDirective => snap
            .no_erc_directives
            .iter()
            .find(|d| d.uuid == item.uuid)
            .map(|d| (d.position.x, d.position.y)),
        SelectedKind::TextNote => snap
            .text_notes
            .iter()
            .find(|t| t.uuid == item.uuid)
            .map(|t| (t.position.x, t.position.y)),
        SelectedKind::ChildSheet => snap
            .child_sheets
            .iter()
            .find(|c| c.uuid == item.uuid)
            .map(|c| (c.position.x, c.position.y)),
        SelectedKind::SheetPin => snap
            .child_sheets
            .iter()
            .find_map(|c| c.pins.iter().find(|pin| pin.uuid == item.uuid))
            .map(|pin| (pin.position.x, pin.position.y)),
        SelectedKind::SymbolRefField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.ref_text.as_ref().map(|rt| (rt.position.x, rt.position.y))),
        SelectedKind::SymbolValField => snap
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(|s| s.val_text.as_ref().map(|vt| (vt.position.x, vt.position.y))),
        _ => None,
    }
}

/// The last ghost, keyed by what it was computed from, so a frame that
/// changes nothing (the cursor moved inside one grid step) does not rerun
/// the rubber-band pass.
pub struct DragGhost {
    key: (Vec<uuid::Uuid>, u64, u64, bool),
    snapshot: std::rc::Rc<SchematicRenderSnapshot>,
}

/// The cell the canvas slot keeps the ghost in; cleared when the document
/// behind it changes.
pub type GhostCell = std::cell::RefCell<Option<DragGhost>>;

/// The ghost for `selection` moved by `(dx, dy)`: with `keep_wires` the very
/// sheet the engine commits (see [`signex_engine::preview_move`]), otherwise
/// the plain shift.
pub(super) fn snapshot(
    cell: &GhostCell,
    snap: &SchematicRenderSnapshot,
    selection: &[SelectedItem],
    dx: f64,
    dy: f64,
    keep_wires: bool,
) -> std::rc::Rc<SchematicRenderSnapshot> {
    let key = (
        selection.iter().map(|item| item.uuid).collect::<Vec<_>>(),
        dx.to_bits(),
        dy.to_bits(),
        keep_wires,
    );
    if let Some(ghost) = cell.borrow().as_ref()
        && ghost.key == key
    {
        return ghost.snapshot.clone();
    }
    let built = if keep_wires {
        let moves: Vec<_> = selection.iter().map(|item| (*item, dx, dy)).collect();
        signex_engine::preview_move(snap, &moves, true)
    } else {
        super::shift_snapshot_for_selection(snap, selection, dx, dy)
    };
    let snapshot = std::rc::Rc::new(built);
    *cell.borrow_mut() = Some(DragGhost {
        key,
        snapshot: snapshot.clone(),
    });
    snapshot
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
    fn the_snapped_delta_puts_the_primary_anchor_on_the_grid() {
        let (sheet, item) = sheet_with_wired_resistor();
        // Symbol origin (50, 50) on a 1.27 grid: 50 / 1.27 is not integral.
        let (dx, dy) = snapped_move_delta(Some(&sheet), &[item], 0.3, 3.1, true, 1.27);
        let (x, y) = (50.0 + dx, 50.0 + dy);
        for v in [x, y] {
            let cells = v / 1.27;
            assert!((cells - cells.round()).abs() < 1e-9, "{v} is off-grid");
        }
        assert_eq!(
            snapped_move_delta(Some(&sheet), &[], 0.3, 3.1, false, 1.27),
            (0.3, 3.1),
            "snap off leaves the delta alone"
        );
    }

    #[test]
    fn the_ghost_rubber_bands_the_wires_when_the_preference_is_on() {
        let (sheet, item) = sheet_with_wired_resistor();
        let cell = GhostCell::default();
        let ghost = snapshot(&cell, &sheet, &[item], 0.0, 10.0, true);
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
        let cell = GhostCell::default();
        let ghost = snapshot(&cell, &sheet, &[item], 0.0, 10.0, false);
        assert_eq!(ghost.symbols[0].position, Point::new(50.0, 60.0));
        assert_eq!(ghost.wires.len(), 1);
        assert_eq!(ghost.wires[0].end, Point::new(45.0, 50.0));
    }
}
