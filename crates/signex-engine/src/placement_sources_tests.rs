use super::*;
use crate::Command;
use crate::test_support::test_sheet;
use signex_types::object_id::SequentialIds;
use signex_types::schematic::{ChildSheet, FillType, LibSymbol, Symbol, TextProp, Wire};

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> SchDrawing {
    SchDrawing::Line {
        uuid: Uuid::now_v7(),
        start: Point::new(x0, y0),
        end: Point::new(x1, y1),
        width: 0.2,
        stroke_color: None,
        locked: false,
    }
}

fn part(at: Point) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".to_owned(),
        reference: "R4".to_owned(),
        value: "10k".to_owned(),
        position: at,
        unit: 1,
        ref_text: Some(TextProp {
            position: Point::new(at.x, at.y - 2.54),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: Default::default(),
            justify_v: Default::default(),
            hidden: false,
        }),
        locked: true,
        ..Default::default()
    }
}

fn sheet_with_a_part() -> SchematicSheet {
    let mut sheet = test_sheet();
    let lib: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
    sheet.lib_symbols.insert(lib.id.clone(), lib);
    sheet.symbols.push(part(Point::new(25.4, 25.4)));
    sheet.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(25.4, 25.4),
        end: Point::new(38.1, 25.4),
        ..Default::default()
    });
    sheet.drawings.push(line(0.0, 0.0, 10.16, 0.0));
    sheet
}

#[test]
fn a_design_block_carries_parts_wires_drawings_and_bodies() {
    let mut sheet = sheet_with_a_part();
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Child".into(),
        filename: "child.snxsch".into(),
        position: Point::new(50.0, 50.0),
        size: (20.0, 20.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let block = DesignBlock::from_sheet(&sheet, 1.27, &mut SequentialIds::default())
        .unwrap()
        .unwrap();
    assert_eq!(block.skipped_sheet_symbols, 1);
    let fragment = block.copied.payload.fragment();
    assert_eq!(fragment.symbols.len(), 1);
    assert_eq!(fragment.wires.len(), 1);
    assert_eq!(fragment.drawings.len(), 1);
    assert!(fragment.child_sheets.is_empty());
    assert!(fragment.body_of(&fragment.symbols[0]).is_some());
}

#[test]
fn a_placed_design_block_is_one_batch_with_fresh_ids() {
    let sheet = sheet_with_a_part();
    let block = DesignBlock::from_sheet(&sheet, 1.27, &mut SequentialIds::default())
        .unwrap()
        .unwrap();
    let commands = block
        .copied
        .payload
        .place_commands(Point::new(100.0, 100.0), &mut SequentialIds::default())
        .unwrap();
    let mut target = Engine::new(test_sheet()).unwrap();
    target.execute_batch(commands).unwrap();
    let doc = target.document();
    assert_eq!(doc.symbols.len(), 1);
    assert_eq!(doc.wires.len(), 1);
    assert_eq!(doc.drawings.len(), 1);
    assert_ne!(doc.drawings[0].uuid(), sheet.drawings[0].uuid());
    // One undo step takes the whole block away.
    target.undo().unwrap();
    let doc = target.document();
    assert!(doc.symbols.is_empty() && doc.wires.is_empty() && doc.drawings.is_empty());
}

#[test]
fn an_empty_sheet_is_no_design_block() {
    assert!(
        DesignBlock::from_sheet(&test_sheet(), 1.27, &mut SequentialIds::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn imported_drawings_are_anchored_at_their_centre_and_turn() {
    let copied = CopiedSelection::from_drawings(
        vec![line(0.0, 0.0, 10.16, 0.0)],
        1.27,
        &mut SequentialIds::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(copied.payload.item_count(), 1);
    let SchDrawing::Line { start, end, .. } = &copied.payload.fragment().drawings[0] else {
        panic!();
    };
    assert!((start.x + end.x).abs() < 1e-9, "centred on the origin");
    let turned = copied.payload.rotate(false).unwrap();
    let SchDrawing::Line { start, end, .. } = &turned.fragment().drawings[0] else {
        panic!();
    };
    assert!(start.x.abs() < 1e-9 && end.x.abs() < 1e-9, "now vertical");
}

#[test]
fn a_part_copy_sits_on_the_origin_with_its_fields_alongside() {
    let engine = Engine::new(sheet_with_a_part()).unwrap();
    let id = engine.document().symbols[0].uuid;
    let payload = engine
        .part_copy_payload(id, "R5".into(), &mut SequentialIds::default())
        .unwrap();
    let copy = payload.lone_symbol().unwrap();
    assert_eq!((copy.position.x, copy.position.y), (0.0, 0.0));
    assert_eq!(copy.reference, "R5");
    assert_eq!(copy.value, "10k");
    assert!(!copy.locked);
    assert_ne!(copy.uuid, id);
    let field = copy.ref_text.as_ref().unwrap().position;
    assert!((field.y + 2.54).abs() < 1e-9, "{field:?}");
    assert!(payload.fragment().body_of(copy).is_some());
    let placed = payload
        .place_commands(Point::new(5.08, 5.08), &mut SequentialIds::default())
        .unwrap();
    assert!(
        placed
            .iter()
            .any(|c| matches!(c, Command::PlaceSymbol { .. }))
    );
}

#[test]
fn no_part_copy_for_an_unknown_id() {
    let engine = Engine::new(sheet_with_a_part()).unwrap();
    assert!(
        engine
            .part_copy_payload(Uuid::now_v7(), "R5".into(), &mut SequentialIds::default())
            .is_none()
    );
}

/// #1283: a selection saved as a snippet holds the circuit and the bodies it
/// uses around the origin, and places again as a design block.
#[test]
fn a_snippet_sheet_is_the_selection_centred_and_places_as_a_design_block() {
    use signex_types::schematic::{SelectedItem, SelectedKind};
    let sheet = sheet_with_a_part();
    let engine = Engine::new(sheet.clone()).unwrap();
    let items = [
        SelectedItem::new(sheet.symbols[0].uuid, SelectedKind::Symbol),
        SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire),
    ];
    let mut ids = SequentialIds::default();
    let snippet = engine.snippet_sheet(&items, &mut ids).unwrap().unwrap();
    assert_eq!(
        snippet.uuid,
        Uuid::from_u128(1),
        "the sheet id comes from `ids`"
    );
    assert_eq!(snippet.symbols.len(), 1);
    assert_eq!(snippet.wires.len(), 1);
    assert!(snippet.drawings.is_empty(), "only what was selected");
    assert!(
        snippet.body_of(&snippet.symbols[0]).is_some(),
        "the body goes along"
    );
    let wire = &snippet.wires[0];
    assert!(
        wire.start.x < 0.0 && wire.end.x > 0.0,
        "centred on the origin: {wire:?}"
    );

    let block = DesignBlock::from_sheet(&snippet, 1.27, &mut SequentialIds::default())
        .unwrap()
        .unwrap();
    let commands = block
        .copied
        .payload
        .place_commands(Point::new(50.8, 50.8), &mut SequentialIds::default())
        .unwrap();
    let mut target = Engine::new(test_sheet()).unwrap();
    target.execute_batch(commands).unwrap();
    assert_eq!(target.document().symbols.len(), 1);
    assert_eq!(target.document().wires.len(), 1);
}

#[test]
fn nothing_selected_is_no_snippet() {
    let engine = Engine::new(sheet_with_a_part()).unwrap();
    assert!(
        engine
            .snippet_sheet(&[], &mut SequentialIds::default())
            .unwrap()
            .is_none()
    );
}
