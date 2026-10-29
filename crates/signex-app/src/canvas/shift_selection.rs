use std::collections::HashSet;

use signex_types::schematic::SchDrawing;

/// Clone the snapshot and translate the world position of every item that
/// appears in `selection` by `(dx, dy)`. Used during drag-to-move so the live
/// render shows selected objects at the cursor position while the originals
/// are not drawn at their pre-drag location. The sheet's library symbols are
/// shared with `snap`, not copied.
pub(super) fn shift_snapshot_for_selection(
    snap: &crate::schematic_runtime::SchematicRenderSnapshot,
    selection: &[signex_types::schematic::SelectedItem],
    dx: f64,
    dy: f64,
) -> crate::schematic_runtime::SchematicRenderSnapshot {
    use signex_types::schematic::{Point, SelectedItem, SelectedKind};

    // One hash lookup per item instead of a scan of the whole selection.
    let selected: HashSet<SelectedItem> = selection.iter().copied().collect();
    let is_selected = |uuid: uuid::Uuid, kind: SelectedKind| -> bool {
        selected.contains(&SelectedItem::new(uuid, kind))
    };
    let shift = |p: Point| -> Point { Point::new(p.x + dx, p.y + dy) };

    let mut out = snap.clone();
    for w in out.wires.iter_mut() {
        if is_selected(w.uuid, SelectedKind::Wire) {
            w.start = shift(w.start);
            w.end = shift(w.end);
        }
    }
    for b in out.buses.iter_mut() {
        if is_selected(b.uuid, SelectedKind::Bus) {
            b.start = shift(b.start);
            b.end = shift(b.end);
        }
    }
    for be in out.bus_entries.iter_mut() {
        if is_selected(be.uuid, SelectedKind::BusEntry) {
            be.position = shift(be.position);
        }
    }
    for j in out.junctions.iter_mut() {
        if is_selected(j.uuid, SelectedKind::Junction) {
            j.position = shift(j.position);
        }
    }
    for nc in out.no_connects.iter_mut() {
        if is_selected(nc.uuid, SelectedKind::NoConnect) {
            nc.position = shift(nc.position);
        }
    }
    for d in out.no_erc_directives.iter_mut() {
        if is_selected(d.uuid, SelectedKind::NoErcDirective) {
            d.position = shift(d.position);
        }
    }
    for l in out.labels.iter_mut() {
        if is_selected(l.uuid, SelectedKind::Label) {
            l.position = shift(l.position);
        }
    }
    for tn in out.text_notes.iter_mut() {
        if is_selected(tn.uuid, SelectedKind::TextNote) {
            tn.position = shift(tn.position);
        }
    }
    for sym in out.symbols.iter_mut() {
        if is_selected(sym.uuid, SelectedKind::Symbol) {
            // Whole-symbol drag: anchor + both fields travel together.
            sym.position = shift(sym.position);
            if let Some(rt) = &mut sym.ref_text {
                rt.position = shift(rt.position);
            }
            if let Some(vt) = &mut sym.val_text {
                vt.position = shift(vt.position);
            }
        } else {
            // Field-only drag: shift just the selected field; the symbol
            // body stays put so the user can reposition the label
            // independently (Altium convention).
            if is_selected(sym.uuid, SelectedKind::SymbolRefField)
                && let Some(rt) = &mut sym.ref_text
            {
                rt.position = shift(rt.position);
            }
            if is_selected(sym.uuid, SelectedKind::SymbolValField)
                && let Some(vt) = &mut sym.val_text
            {
                vt.position = shift(vt.position);
            }
        }
    }
    for cs in out.child_sheets.iter_mut() {
        if is_selected(cs.uuid, SelectedKind::ChildSheet) {
            cs.position = shift(cs.position);
            for pin in &mut cs.pins {
                pin.position = shift(pin.position);
            }
        } else {
            for pin in &mut cs.pins {
                if is_selected(pin.uuid, SelectedKind::SheetPin) {
                    pin.position = shift(pin.position);
                }
            }
        }
    }
    for d in out.drawings.iter_mut() {
        let uuid = match d {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        if !is_selected(uuid, SelectedKind::Drawing) {
            continue;
        }
        match d {
            SchDrawing::Line { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Rect { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Circle { center, .. } => {
                *center = shift(*center);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                *start = shift(*start);
                *mid = shift(*mid);
                *end = shift(*end);
            }
            SchDrawing::Polyline { points, .. } => {
                for p in points {
                    *p = shift(*p);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        Junction, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
    };

    fn label(x: f64) -> signex_types::schematic::Label {
        serde_json::from_value(serde_json::json!({
            "uuid": uuid::Uuid::new_v4(),
            "text": "N",
            "position": { "x": x, "y": 1.0 },
            "label_type": "net",
        }))
        .expect("label")
    }

    /// Two of each kind; the first of each is dragged, the second is not.
    fn sheet_with_pairs() -> (SchematicSheet, Vec<SelectedItem>) {
        let mut sheet = SchematicSheet::default();
        let mut selected = Vec::new();
        for n in 0..2 {
            let o = f64::from(n) * 100.0;
            let symbol = Symbol {
                uuid: uuid::Uuid::new_v4(),
                position: Point::new(o, 10.0),
                ..Symbol::default()
            };
            let wire = Wire {
                uuid: uuid::Uuid::new_v4(),
                start: Point::new(o, 0.0),
                end: Point::new(o + 5.0, 0.0),
                ..Wire::default()
            };
            let junction = Junction {
                uuid: uuid::Uuid::new_v4(),
                position: Point::new(o + 2.0, 0.0),
                ..Junction::default()
            };
            let lbl = label(o);
            let drawing = SchDrawing::Line {
                uuid: uuid::Uuid::new_v4(),
                start: Point::new(o, 20.0),
                end: Point::new(o + 3.0, 21.0),
                width: 0.0,
                stroke_color: None,
                locked: false,
            };
            if n == 0 {
                let SchDrawing::Line { uuid, .. } = &drawing else {
                    unreachable!()
                };
                selected.extend([
                    SelectedItem::new(symbol.uuid, SelectedKind::Symbol),
                    SelectedItem::new(wire.uuid, SelectedKind::Wire),
                    SelectedItem::new(junction.uuid, SelectedKind::Junction),
                    SelectedItem::new(lbl.uuid, SelectedKind::Label),
                    SelectedItem::new(*uuid, SelectedKind::Drawing),
                ]);
            }
            sheet.symbols.push(symbol);
            sheet.wires.push(wire);
            sheet.junctions.push(junction);
            sheet.labels.push(lbl);
            sheet.drawings.push(drawing);
        }
        (sheet, selected)
    }

    #[test]
    fn a_mixed_selection_moves_and_the_rest_stays() {
        let (sheet, selected) = sheet_with_pairs();
        let out = shift_snapshot_for_selection(&sheet, &selected, 3.0, -2.0);

        assert_eq!(out.symbols[0].position, Point::new(3.0, 8.0));
        assert_eq!(out.wires[0].start, Point::new(3.0, -2.0));
        assert_eq!(out.wires[0].end, Point::new(8.0, -2.0));
        assert_eq!(out.junctions[0].position, Point::new(5.0, -2.0));
        assert_eq!(out.labels[0].position, Point::new(3.0, -1.0));
        match &out.drawings[0] {
            SchDrawing::Line { start, end, .. } => {
                assert_eq!(*start, Point::new(3.0, 18.0));
                assert_eq!(*end, Point::new(6.0, 19.0));
            }
            other => panic!("unexpected drawing {other:?}"),
        }

        // The unselected second of each kind is untouched.
        assert_eq!(out.symbols[1].position, sheet.symbols[1].position);
        assert_eq!(out.wires[1].start, sheet.wires[1].start);
        assert_eq!(out.junctions[1].position, sheet.junctions[1].position);
        assert_eq!(out.labels[1].position, sheet.labels[1].position);
    }

    #[test]
    fn a_kind_mismatch_does_not_select() {
        let (sheet, _) = sheet_with_pairs();
        let wrong = [SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Label)];
        let out = shift_snapshot_for_selection(&sheet, &wrong, 3.0, 3.0);
        assert_eq!(out.wires[0].start, sheet.wires[0].start);
    }

    #[test]
    fn the_shifted_snapshot_shares_the_library_symbols_instead_of_copying_them() {
        let (sheet, selected) = sheet_with_pairs();
        let out = shift_snapshot_for_selection(&sheet, &selected, 1.0, 1.0);
        assert!(out.lib_symbols.shares_storage_with(&sheet.lib_symbols));
    }
}
