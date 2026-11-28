//! #1667 — the operand forms of the placement extras: the file is read off
//! the UI thread, the content lands in one undo step, the answer says what
//! was placed, and the key reaches the same handler.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;

use serde_json::{Value, json};
use signex_types::schematic::{
    HAlign, LibSymbol, Point, SchDrawing, SchematicSheet, Symbol, TextNote, TextProp, VAlign, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, core_to_message};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, command: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(command).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands");
    app.run_operands_settled(&command, &args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

fn app_in(dir: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 2.54;
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet);
    app
}

/// A block file: one wire, one text note and one line.
fn write_block(dir: &Path) -> PathBuf {
    let mut block = empty_sheet();
    block.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.16, 0.0),
        ..Default::default()
    });
    block.text_notes.push(TextNote {
        uuid: Uuid::now_v7(),
        text: "note".into(),
        position: Point::new(5.08, 5.08),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
        format: Default::default(),
        rich: Default::default(),
    });
    block.drawings.push(SchDrawing::Line {
        uuid: Uuid::now_v7(),
        start: Point::new(0.0, 10.16),
        end: Point::new(10.16, 10.16),
        width: 0.2,
        stroke_color: None,
        locked: false,
    });
    let text = signex_types::format::SnxSchematic::new(block)
        .write_string()
        .expect("the block encodes");
    let path = dir.join("block.snxsch");
    std::fs::write(&path, text).expect("block written");
    path
}

#[test]
fn place_design_block_places_the_file_content_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let block = write_block(dir.path());
    let mut app = app_in(dir.path(), empty_sheet());

    let answer = answered(run(
        &mut app,
        "place_design_block",
        &[format!("path={}", block.display()), "at=50.8,50.8".into()],
    ));

    assert_eq!(answer, json!({"placed": 3, "skipped_sheet_symbols": 0}));
    assert_eq!(doc(&app).wires.len(), 1);
    assert_eq!(doc(&app).text_notes.len(), 1);
    assert_eq!(doc(&app).drawings.len(), 1);
    undo(&mut app);
    let sheet = doc(&app);
    assert!(sheet.wires.is_empty() && sheet.text_notes.is_empty() && sheet.drawings.is_empty());
}

#[test]
fn place_design_block_off_the_grid_is_refused_and_places_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let block = write_block(dir.path());
    let mut app = app_in(dir.path(), empty_sheet());

    let reason = refusal(run(
        &mut app,
        "place_design_block",
        &[format!("path={}", block.display()), "at=50.9,50.8".into()],
    ));

    assert!(reason.contains("not where a click lands"), "{reason}");
    assert!(doc(&app).wires.is_empty());
}

#[test]
fn place_design_block_of_a_missing_file_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path(), empty_sheet());
    let missing = dir.path().join("nowhere.snxsch");

    let reason = refusal(run(
        &mut app,
        "place_design_block",
        &[format!("path={}", missing.display()), "at=50.8,50.8".into()],
    ));

    assert!(reason.contains("no file"), "{reason}");
}

#[test]
fn import_graphics_places_the_drawings_and_counts_what_it_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dxf = dir.path().join("logo.dxf");
    std::fs::write(
        &dxf,
        "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10.16\n21\n0\n\
         0\nTEXT\n10\n0\n20\n0\n1\nhi\n0\nENDSEC\n0\nEOF\n",
    )
    .expect("dxf written");
    let mut app = app_in(dir.path(), empty_sheet());

    let answer = answered(run(
        &mut app,
        "import_graphics",
        &[format!("path={}", dxf.display()), "at=50.8,50.8".into()],
    ));

    assert_eq!(answer, json!({"placed": 1, "skipped": 1}));
    let [SchDrawing::Line { start, end, .. }] = doc(&app).drawings.as_slice() else {
        panic!("{:?}", doc(&app).drawings);
    };
    assert!((end.x - start.x - 10.16).abs() < 1e-9, "the length is kept");
    undo(&mut app);
    assert!(doc(&app).drawings.is_empty());
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

#[test]
fn autoplace_fields_names_its_part_and_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut sheet = empty_sheet();
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
    sheet.lib_symbols.insert(body.id.clone(), body);
    let id = Uuid::now_v7();
    sheet.symbols.push(Symbol {
        uuid: id,
        lib_id: "lib:r".into(),
        reference: "R1".into(),
        position: Point::new(50.8, 50.8),
        unit: 1,
        ref_text: Some(field(150.0, 10.0)),
        fields_user_placed: true,
        ..Default::default()
    });
    let mut app = app_in(dir.path(), sheet);
    let document = dir.path().join("main.snxsch");

    let answer = answered(run(
        &mut app,
        "autoplace_fields",
        &[
            format!("id={id}"),
            format!("document={}", document.display()),
        ],
    ));

    assert_eq!(answer, json!({"id": id.to_string(), "moved": true}));
    assert!(!doc(&app).symbols[0].fields_user_placed);

    let unknown = refusal(run(
        &mut app,
        "autoplace_fields",
        &[
            format!("id={}", Uuid::now_v7()),
            format!("document={}", document.display()),
        ],
    ));
    assert!(unknown.contains("no part"), "{unknown}");
}

/// The keys reach the handlers the operand forms share.
#[test]
fn the_placement_extra_ids_reach_their_messages() {
    let resolve = |id: &str| {
        let command = AppCommandId::new(id).expect("a catalog id");
        core_to_message(&command, &CommandArgs::none())
            .unwrap_or_else(|| panic!("'{id}' does not resolve"))
    };
    let extra = |message: Message| match message {
        Message::Edit(EditMsg::Extras(msg)) => msg,
        other => panic!("not a placement extra: {other:?}"),
    };
    assert!(matches!(
        extra(resolve("autoplace_fields")),
        PlacementExtrasMsg::AutoplaceFieldsRequested
    ));
    assert!(matches!(
        extra(resolve("copy_attributes_or_add_vertex")),
        PlacementExtrasMsg::CopyAttributesRequested { at: None }
    ));
    assert!(matches!(
        extra(resolve("place_design_block")),
        PlacementExtrasMsg::DesignBlockRequested
    ));
    assert!(matches!(
        extra(resolve("import_graphics")),
        PlacementExtrasMsg::ImportGraphicsRequested
    ));
}
