//! #2953: `set_object_text` sets a part's designator or value, a label's name
//! or a note's text from operands (the CLI and agent path), as one engine
//! edit and one undo step, the edit the Properties rows send.

use std::path::Path;

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{
    HAlign, Label, LabelType, Point, SchematicSheet, Symbol, TextNote, VAlign,
};
use uuid::Uuid;

use super::text_target;
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;

const PART: u128 = 1;
const PORT: u128 = 2;
const LABEL: u128 = 3;
const NOTE: u128 = 4;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// R1 (10k), a power port VCC, a net label NET and a note.
fn sheet() -> SchematicSheet {
    let mut label = Label::new("NET", Point::new(100.0, 10.0), LabelType::Net);
    label.uuid = id(LABEL);
    SchematicSheet {
        paper_size: "A4".into(),
        symbols: vec![
            Symbol {
                uuid: id(PART),
                lib_id: "Device:R".into(),
                reference: "R1".into(),
                value: "10k".into(),
                position: Point::new(10.0, 10.0),
                ..Default::default()
            },
            Symbol {
                uuid: id(PORT),
                lib_id: "power:VCC".into(),
                reference: "#PWR01".into(),
                value: "VCC".into(),
                is_power: true,
                position: Point::new(40.0, 10.0),
                ..Default::default()
            },
        ],
        labels: vec![label],
        text_notes: vec![TextNote {
            uuid: id(NOTE),
            text: "note".into(),
            position: Point::new(150.0, 10.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: false,
        }],
        ..Default::default()
    }
}

fn app_with(dir: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet);
    app
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn run(app: &mut Signex, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new("set_object_text").expect("a catalog id");
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

fn undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

#[test]
fn each_field_is_set_in_one_undo_step_and_answered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let of = |n: u128| format!("id={}", id(n));
    for (n, field, value) in [
        (PART, "designator", "R7"),
        (PART, "value", "4k7"),
        (PORT, "value", "+3V3"),
        (LABEL, "text", "SDA"),
        (NOTE, "text", "Place C4 near U1"),
    ] {
        let answer = answered(run(
            &mut app,
            &[of(n), format!("field={field}"), format!("value={value}")],
        ));
        assert_eq!(answer["id"], id(n).to_string());
        assert_eq!(answer["text"]["field"], field, "{answer}");
        assert_eq!(answer["text"]["value"], value, "{answer}");
    }
    let sheet = doc(&app);
    assert_eq!(
        (
            sheet.symbols[0].reference.as_str(),
            sheet.symbols[0].value.as_str()
        ),
        ("R7", "4k7")
    );
    assert_eq!(sheet.symbols[1].value, "+3V3");
    assert_eq!(sheet.labels[0].text, "SDA");
    assert_eq!(sheet.text_notes[0].text, "Place C4 near U1");

    // One undo step per edit, back to the start.
    for _ in 0..5 {
        undo(&mut app);
    }
    let sheet = doc(&app);
    assert_eq!(sheet.symbols[0].reference, "R1");
    assert_eq!(sheet.symbols[0].value, "10k");
    assert_eq!(sheet.symbols[1].value, "VCC");
    assert_eq!(sheet.labels[0].text, "NET");
    assert_eq!(sheet.text_notes[0].text, "note");
}

#[test]
fn a_field_the_object_does_not_have_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut locked = sheet();
    locked.symbols[0].locked = true;
    let mut app = app_with(dir.path(), locked);
    let of = |n: u128| format!("id={}", id(n));
    for (operands, needle) in [
        (vec![of(LABEL), "field=value".into()], "no `value`"),
        (vec![of(PORT), "field=designator".into()], "no designator"),
        (vec![of(PART), "field=colour".into()], "one of designator"),
        (
            vec![of(PART), "field=value".into(), "value=1k".into()],
            "locked",
        ),
    ] {
        let reason = refusal(run(&mut app, &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
    }
    assert_eq!(doc(&app).symbols[0].value, "10k");
    assert_eq!(doc(&app).labels[0].text, "NET");
}

#[test]
fn the_target_names_the_field_of_each_kind() {
    use signex_engine::TextTarget;
    use signex_types::schematic::{SelectedItem, SelectedKind};
    let s = sheet();
    let item = |n, kind| SelectedItem::new(id(n), kind);
    assert_eq!(
        text_target(&s, item(PART, SelectedKind::Symbol), " designator "),
        Ok(TextTarget::SymbolReference(id(PART)))
    );
    assert_eq!(
        text_target(&s, item(NOTE, SelectedKind::TextNote), "text"),
        Ok(TextTarget::TextNote(id(NOTE)))
    );
    assert!(text_target(&s, item(NOTE, SelectedKind::TextNote), "value").is_err());
}

/// Without operands (palette, key) the command says where to edit.
#[test]
fn without_operands_the_command_names_the_object_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet());
    let command = AppCommandId::new("set_object_text").expect("id");
    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert!(refusal(outcome).contains("Object List"));
}
