//! #1667 — placement extras on a sheet: Autoplace Fields, Copy Attributes,
//! Place Design Block and Import Graphics, driven through `Signex::update`.

use std::path::PathBuf;
use std::sync::Arc;

use signex_app::app::{EditMsg, Message, PlacementExtrasMsg, Signex, Tool};
use signex_types::schematic::{
    HAlign, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchDrawing, SchematicSheet,
    SelectedItem, SelectedKind, TextProp, VAlign,
};

use crate::paste_support::sheet::{
    SHEET, app_with_sheet, click, part, populated_sheet, references, select,
};

fn extras(app: &mut Signex, msg: PlacementExtrasMsg) -> iced::Task<Message> {
    app.update(Message::Edit(EditMsg::Extras(msg)))
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state.active_engine().unwrap().document()
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn field(x: f64, y: f64) -> TextProp {
    TextProp {
        position: Point::new(x, y),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    }
}

/// A one-pin part `P1` whose fields the user dragged far off its body.
fn sheet_with_moved_fields() -> SchematicSheet {
    let mut sheet = populated_sheet();
    let mut body: LibSymbol = serde_json::from_str(r#"{"id":"lib:p"}"#).unwrap();
    body.pins.push(LibPin {
        unit: 1,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, 3.81),
            rotation: 270.0,
            length: 2.54,
            name: "1".into(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    });
    sheet.lib_symbols.insert(body.id.clone(), body);
    let mut moved = part("P1", "lib:p", 1, (101.6, 101.6));
    moved.ref_text = Some(field(180.0, 20.0));
    moved.val_text = Some(field(180.0, 22.0));
    moved.fields_user_placed = true;
    sheet.symbols.push(moved);
    sheet
}

fn p1(app: &Signex) -> signex_types::schematic::Symbol {
    doc(app)
        .symbols
        .iter()
        .find(|s| s.reference == "P1")
        .cloned()
        .expect("P1 on the sheet")
}

#[test]
fn autoplace_fields_brings_moved_fields_back_to_the_part_in_one_undo_step() {
    let mut app = app_with_sheet(sheet_with_moved_fields());
    let before = p1(&app);
    select(&mut app, &[("P1", 1)], false);

    let _ = extras(&mut app, PlacementExtrasMsg::AutoplaceFieldsRequested);

    let after = p1(&app);
    assert!(!after.fields_user_placed, "the fields count as autoplaced");
    let at = after.ref_text.as_ref().unwrap().position;
    assert!(
        (at.x - 101.6).abs() < 20.0 && (at.y - 101.6).abs() < 20.0,
        "the reference sits by its part: {at:?}"
    );
    undo(&mut app);
    assert_eq!(p1(&app), before, "one undo step puts them back");
}

#[test]
fn autoplace_fields_with_no_part_selected_changes_nothing() {
    let mut app = app_with_sheet(sheet_with_moved_fields());
    let before = p1(&app);
    let _ = extras(&mut app, PlacementExtrasMsg::AutoplaceFieldsRequested);
    assert_eq!(p1(&app), before);
}

/// A locked part in the selection keeps its fields; the others are placed.
#[test]
fn autoplace_fields_skips_a_locked_part_and_places_the_rest() {
    let mut sheet = sheet_with_moved_fields();
    let mut locked = part("P2", "lib:p", 1, (152.4, 101.6));
    locked.ref_text = Some(field(200.0, 20.0));
    locked.fields_user_placed = true;
    locked.locked = true;
    sheet.symbols.push(locked);
    let mut app = app_with_sheet(sheet);
    select(&mut app, &[("P1", 1), ("P2", 1)], false);
    let p2 = |app: &Signex| {
        doc(app)
            .symbols
            .iter()
            .find(|s| s.reference == "P2")
            .cloned()
            .expect("P2 on the sheet")
    };
    let before = p2(&app);

    let _ = extras(&mut app, PlacementExtrasMsg::AutoplaceFieldsRequested);

    assert!(!p1(&app).fields_user_placed, "the unlocked part is placed");
    assert_eq!(p2(&app), before, "the locked part keeps its fields");
}

#[test]
fn copy_attributes_on_a_label_arms_the_label_tool_with_its_text() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[], true);

    let _ = extras(
        &mut app,
        PlacementExtrasMsg::CopyAttributesRequested { at: None },
    );

    assert_eq!(app.interaction_state.current_tool, Tool::Label);
    let form = app.document_state.panel_ctx.pre_placement.as_ref().unwrap();
    assert_eq!(form.label_text, "D0");
    let ghost = app.interaction_state.active_canvas().ghost_label.as_ref();
    assert_eq!(
        ghost.map(|label| label.text.as_str()),
        Some("D0"),
        "the copy follows the cursor"
    );
    click(&mut app, (60.96, 60.96));
    let texts: Vec<&str> = doc(&app).labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert_eq!(texts[1], "D0");
}

#[test]
fn copy_attributes_on_a_part_places_another_with_the_next_designator() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], false);

    let _ = extras(
        &mut app,
        PlacementExtrasMsg::CopyAttributesRequested { at: None },
    );
    assert!(
        app.interaction_state.placement.is_some(),
        "a part is on the cursor"
    );

    click(&mut app, (60.96, 60.96));
    let placed = doc(&app).symbols.last().cloned().unwrap();
    assert_eq!(placed.reference, "R4");
    assert_eq!(placed.lib_id, "lib:r");
    assert!(
        (placed.position.x - 60.96).abs() < 1e-9 && (placed.position.y - 60.96).abs() < 1e-9,
        "the part's origin is where the click was: {:?}",
        placed.position
    );
}

#[test]
fn copy_attributes_while_a_wire_is_drawn_leaves_the_tool_alone() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[], true);
    app.interaction_state.wire_drawing = true;
    let tool = app.interaction_state.current_tool;

    let _ = extras(
        &mut app,
        PlacementExtrasMsg::CopyAttributesRequested { at: None },
    );

    assert_eq!(app.interaction_state.current_tool, tool);
    assert!(app.interaction_state.placement.is_none());
}

/// The block as the reader hands it back: the populated sheet with the
/// bodies its parts use.
fn block_sheet() -> Arc<SchematicSheet> {
    let mut sheet = populated_sheet();
    for lib in ["lib:r", "lib:dual"] {
        let body: LibSymbol = serde_json::from_str(&format!(r#"{{"id":"{lib}"}}"#)).unwrap();
        sheet.lib_symbols.insert(body.id.clone(), body);
    }
    Arc::new(sheet)
}

fn design_block_read(app: &mut Signex, epoch: u64) {
    let _ = extras(
        app,
        PlacementExtrasMsg::DesignBlockRead {
            path: PathBuf::from("block.snxsch"),
            target: PathBuf::from(SHEET),
            epoch,
            sheet: Ok(block_sheet()),
        },
    );
}

#[test]
fn a_design_block_lands_on_the_cursor_and_one_click_places_it_as_one_step() {
    let mut app = app_with_sheet(populated_sheet());
    let epoch = app.interaction_state.placement_epoch;
    design_block_read(&mut app, epoch);
    assert!(
        app.interaction_state.placement.is_some(),
        "the block is on the cursor"
    );

    click(&mut app, (101.6, 101.6));
    assert_eq!(doc(&app).symbols.len(), 10, "{:?}", references(&app));
    assert_eq!(doc(&app).labels.len(), 2);
    assert!(app.interaction_state.placement.is_none(), "placed once");

    undo(&mut app);
    assert_eq!(doc(&app).symbols.len(), 5);
    assert_eq!(doc(&app).labels.len(), 1);
}

#[test]
fn a_design_block_read_after_the_tool_changed_is_dropped() {
    let mut app = app_with_sheet(populated_sheet());
    let stale = app.interaction_state.placement_epoch;
    app.interaction_state.placement_epoch += 1;
    design_block_read(&mut app, stale);
    assert!(app.interaction_state.placement.is_none());
}

const DXF: &str = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n10\n0\n20\n0\n11\n10.16\n21\n0\n\
0\nCIRCLE\n10\n5.08\n20\n5.08\n40\n2.54\n\
0\nTEXT\n10\n0\n20\n0\n1\nlabel\n\
0\nENDSEC\n0\nEOF\n";

#[test]
fn import_graphics_reads_the_file_off_the_ui_thread_and_places_its_drawings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logo.dxf");
    std::fs::write(&path, DXF).unwrap();
    let mut app = app_with_sheet(populated_sheet());

    let task = extras(
        &mut app,
        PlacementExtrasMsg::ImportGraphicsPicked(Some(path)),
    );
    for message in crate::library_place_symbol::run(task) {
        let _ = app.update(message);
    }
    assert!(
        app.interaction_state.placement.is_some(),
        "the drawings are on the cursor"
    );

    click(&mut app, (101.6, 101.6));
    let drawings = &doc(&app).drawings;
    assert_eq!(drawings.len(), 2, "the text is skipped: {drawings:?}");
    assert!(matches!(drawings[0], SchDrawing::Line { .. }));
    assert!(matches!(drawings[1], SchDrawing::Circle { .. }));

    undo(&mut app);
    assert!(doc(&app).drawings.is_empty());
}

#[test]
fn import_graphics_of_a_file_that_is_not_dxf_places_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.dxf");
    std::fs::write(&path, "hello\nworld\nthird\n").unwrap();
    let mut app = app_with_sheet(populated_sheet());

    let task = extras(
        &mut app,
        PlacementExtrasMsg::ImportGraphicsPicked(Some(path)),
    );
    for message in crate::library_place_symbol::run(task) {
        let _ = app.update(message);
    }
    assert!(app.interaction_state.placement.is_none());
    assert!(doc(&app).drawings.is_empty());
}

#[test]
fn a_selected_reference_field_stands_for_its_part() {
    let mut app = app_with_sheet(sheet_with_moved_fields());
    let id = p1(&app).uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::SymbolRefField)];
    let _ = extras(&mut app, PlacementExtrasMsg::AutoplaceFieldsRequested);
    assert!(!p1(&app).fields_user_placed);
}
