//! #1114, mockup row 203: a text note's Format row (Plain / Rich text) and
//! `set_text_note_format`. The Properties pick (through `Signex::update`),
//! the operand run and the headless run make the same edit: one undo step,
//! the text kept as it is, a lock refusing with its reason.

use std::path::Path;

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{
    HAlign, Point, SchematicSheet, SelectedItem, SelectedKind, TextNote, TextNoteFormat, VAlign,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;
use crate::panels::{NoteLayout, PanelMsg, PlacedEditMsg};

const SOURCE: &str = "= Layout notes\n*Keep the hot loop small.*\n$V = 0.8 dot (1 + R_7 / R_8)$";

fn note_id() -> Uuid {
    Uuid::from_u128(6)
}

fn sheet(locked: bool) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        text_notes: vec![TextNote {
            uuid: note_id(),
            text: SOURCE.into(),
            position: Point::new(150.0, 10.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked,
            format: TextNoteFormat::Plain,
            rich: Default::default(),
        }],
        ..Default::default()
    }
}

fn app_with(dir: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet);
    app
}

fn note(app: &Signex) -> &TextNote {
    &app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .text_notes[0]
}

fn item() -> SelectedItem {
    SelectedItem::new(note_id(), SelectedKind::TextNote)
}

fn pick(app: &mut Signex, format: TextNoteFormat) {
    let msg = PlacedEditMsg::NoteLayoutPicked {
        item: item(),
        layout: NoteLayout::Format(format),
    };
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::PlacedEdit(msg),
    )));
}

fn undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

fn args(command: &AppCommandId, format: &str) -> CommandArgs {
    let id = format!("id={}", note_id());
    let format = format!("format={format}");
    CommandArgs::parse_operands(command, &[id.as_str(), format.as_str()]).expect("operands")
}

fn command() -> AppCommandId {
    AppCommandId::new("set_text_note_format").expect("a catalog id")
}

/// The CLI and agent path.
fn run(app: &mut Signex, format: &str) -> CommandOutcome {
    let command = command();
    let args = args(&command, format);
    app.run_operands_settled(&command, &args).0
}

/// A headless session's path.
fn run_headless(app: &mut Signex, format: &str) -> CommandOutcome {
    let command = command();
    let args = args(&command, format);
    app.run_headless_command_in_test(&command, args).0
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

/// The Format row switches a selected note to Rich text and back, one undo
/// step each, and the source is never touched.
#[test]
fn the_format_row_switches_the_note_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet(false));
    app.interaction_state.active_canvas_mut().selected = vec![item()];
    app.update_selection_info();

    pick(&mut app, TextNoteFormat::Typst);
    assert_eq!(note(&app).format, TextNoteFormat::Typst);
    assert_eq!(note(&app).text, SOURCE);
    pick(&mut app, TextNoteFormat::Typst);
    pick(&mut app, TextNoteFormat::Plain);
    assert_eq!(note(&app).format, TextNoteFormat::Plain);
    assert_eq!(note(&app).text, SOURCE, "switching back keeps the source");

    undo(&mut app);
    assert_eq!(
        note(&app).format,
        TextNoteFormat::Typst,
        "one step per pick"
    );
    undo(&mut app);
    assert_eq!(note(&app).format, TextNoteFormat::Plain);
}

#[test]
fn the_command_sets_the_format_and_answers_with_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet(false));
    let answer = answered(run(&mut app, "typst"));
    assert_eq!(answer["id"], note_id().to_string());
    assert_eq!(answer["kind"], "text");
    assert_eq!(answer["format"], "typst");
    assert_eq!(note(&app).format, TextNoteFormat::Typst);
    assert_eq!(note(&app).text, SOURCE);
    undo(&mut app);
    assert_eq!(note(&app).format, TextNoteFormat::Plain);
}

#[test]
fn a_headless_run_sets_the_format_as_the_row_does() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet(false));
    let answer = answered(run_headless(&mut app, "typst"));
    assert_eq!(answer["format"], "typst");
    assert_eq!(note(&app).format, TextNoteFormat::Typst);
    let back = answered(run_headless(&mut app, "plain"));
    assert_eq!(back["format"], "plain");
    assert_eq!(note(&app).format, TextNoteFormat::Plain);
    assert_eq!(note(&app).text, SOURCE);
}

#[test]
fn the_command_refuses_with_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet(false));
    let reason = refusal(run(&mut app, "markdown"));
    assert!(reason.contains("`plain` or `typst`"), "{reason}");
    assert_eq!(note(&app).format, TextNoteFormat::Plain);

    let mut locked = app_with(dir.path(), sheet(true));
    let reason = refusal(run(&mut locked, "typst"));
    assert!(reason.contains("locked"), "{reason}");
    assert_eq!(note(&locked).format, TextNoteFormat::Plain);
}

/// Without operands (palette, key) the command says which row to use.
#[test]
fn without_operands_the_command_names_the_format_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), sheet(false));
    let (outcome, _task) = app.dispatch_command_reporting(&command(), CommandArgs::none(), None);
    let reason = refusal(outcome);
    assert!(reason.contains("Format row"), "{reason}");
}
