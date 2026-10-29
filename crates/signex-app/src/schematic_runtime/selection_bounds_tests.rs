use super::*;
use signex_types::schematic::{Junction, Wire};

fn label(text: &str, x: f64, y: f64) -> Label {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::now_v7(),
        "text": text,
        "position": { "x": x, "y": y },
        "label_type": "net",
    }))
    .expect("label")
}

/// A sheet with every kind the overlay boxes: symbol, wire, label, drawing,
/// junction, plus a few extras so lookups cannot succeed by position alone.
fn mixed_sheet() -> (SchematicSheet, Vec<SelectedItem>) {
    let mut sheet = SchematicSheet::default();
    let mut selected = Vec::new();
    for n in 0..4 {
        let o = f64::from(n) * 10.0;
        let sym = Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "missing:lib".into(),
            position: Point::new(o, 5.0),
            ..Symbol::default()
        };
        let wire = Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(o, 0.0),
            end: Point::new(o + 4.0, 0.0),
            ..Wire::default()
        };
        let junction = Junction {
            uuid: uuid::Uuid::now_v7(),
            position: Point::new(o + 2.0, 0.0),
            ..Junction::default()
        };
        let lbl = label("N", o, 2.0);
        let drawing = SchDrawing::Line {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(o, 8.0),
            end: Point::new(o + 3.0, 9.0),
            width: 0.0,
            stroke_color: None,
        };
        if n % 2 == 0 {
            selected.extend([
                SelectedItem::new(sym.uuid, SelectedKind::Symbol),
                SelectedItem::new(wire.uuid, SelectedKind::Wire),
                SelectedItem::new(junction.uuid, SelectedKind::Junction),
                SelectedItem::new(lbl.uuid, SelectedKind::Label),
                SelectedItem::new(drawing_uuid(&drawing), SelectedKind::Drawing),
            ]);
        }
        sheet.symbols.push(sym);
        sheet.wires.push(wire);
        sheet.junctions.push(junction);
        sheet.labels.push(lbl);
        sheet.drawings.push(drawing);
    }
    (sheet, selected)
}

fn drawing_uuid(drawing: &SchDrawing) -> uuid::Uuid {
    match drawing {
        SchDrawing::Line { uuid, .. }
        | SchDrawing::Rect { uuid, .. }
        | SchDrawing::Circle { uuid, .. }
        | SchDrawing::Arc { uuid, .. }
        | SchDrawing::Polyline { uuid, .. } => *uuid,
    }
}

fn corners(bbox: Option<Aabb>) -> Option<[f64; 4]> {
    bbox.map(|b| [b.min_x, b.min_y, b.max_x, b.max_y])
}

/// The per-item lookup the overlay used before the batch entry point: build
/// every bound, then find the one item.
fn reference_aabb(sheet: &SchematicSheet, item: &SelectedItem) -> Option<Aabb> {
    collect_item_bounds(sheet, &Default::default())
        .into_iter()
        .find(|entry| entry.item == *item)
        .map(|entry| entry.bbox)
}

#[test]
fn batch_boxes_equal_the_per_item_lookup_for_a_mixed_selection() {
    let (sheet, selected) = mixed_sheet();
    let batch = selected_aabbs(&sheet, &selected, &Default::default());
    assert_eq!(batch.len(), selected.len());
    for (item, got) in selected.iter().zip(&batch) {
        let want = reference_aabb(&sheet, item);
        assert!(want.is_some(), "{item:?} has a reference box");
        assert_eq!(corners(*got), corners(want), "{item:?}");
    }
}

#[test]
fn batch_boxes_stay_aligned_when_an_item_is_gone() {
    let (sheet, mut selected) = mixed_sheet();
    selected.insert(
        1,
        SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Wire),
    );
    selected.push(SelectedItem::new(
        uuid::Uuid::now_v7(),
        SelectedKind::Symbol,
    ));
    let batch = selected_aabbs(&sheet, &selected, &Default::default());
    assert_eq!(batch.len(), selected.len());
    assert!(batch[1].is_none());
    assert!(batch.last().expect("last").is_none());
    assert_eq!(
        corners(batch[0]),
        corners(reference_aabb(&sheet, &selected[0]))
    );
    assert_eq!(
        corners(batch[2]),
        corners(reference_aabb(&sheet, &selected[2]))
    );
}
