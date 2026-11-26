//! #1114, mockup row 203: the rich note catalog commands, on the CLI and
//! agent path (`run_operands_settled`) and the headless path, each the same
//! edit or action as its row, button or key, answered with JSON.

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{NoteFrame, NoteSize, TextNote, TextNoteFormat};

use super::tests::{SOURCE, app_with, note, note_id, rich_note};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn args(command: &AppCommandId, operands: &[&str]) -> CommandArgs {
    let id = format!("id={}", note_id());
    let mut all = vec![id.as_str()];
    all.extend_from_slice(operands);
    CommandArgs::parse_operands(command, &all).expect("operands parse")
}

/// The CLI and agent path.
fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = args(&command, operands);
    let (outcome, task) = app.run_operands_settled(&command, &args);
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
    outcome
}

/// A headless session's path.
fn run_headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = args(&command, operands);
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

fn undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

#[test]
fn set_text_note_frame_sets_the_section_and_answers_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let answer = answered(run(
        &mut app,
        "set_text_note_frame",
        &["frame=border", "border=0.25", "padding=1"],
    ));
    assert_eq!(answer["frame"], "border");
    assert_eq!(answer["border_mm"], 0.25);
    assert_eq!(answer["padding_mm"], 1.0);
    assert_eq!(note(&app).rich.frame, NoteFrame::Border);
    let auto = answered(run_headless(
        &mut app,
        "set_text_note_frame",
        &["frame=filled", "border=auto"],
    ));
    assert_eq!(auto["border_mm"], Value::Null, "Auto");
    assert_eq!(note(&app).rich.frame, NoteFrame::Filled);
    assert_eq!(note(&app).text, SOURCE);
    undo(&mut app);
    assert_eq!(note(&app).rich.frame, NoteFrame::Border, "one step each");
}

#[test]
fn set_text_note_size_sets_the_section_and_refuses_a_bad_word() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let answer = answered(run(
        &mut app,
        "set_text_note_size",
        &["sizing=fixed_box", "width=60", "height=20"],
    ));
    assert_eq!(answer["sizing"], "fixed_box");
    assert_eq!(note(&app).rich.size, NoteSize::FixedBox);
    assert!((note(&app).rich.height_mm - 20.0).abs() < 1e-9);
    let headless = answered(run_headless(
        &mut app,
        "set_text_note_size",
        &["sizing=fit_text"],
    ));
    assert_eq!(headless["sizing"], "fit_text");
    let reason = refusal(run(&mut app, "set_text_note_size", &["sizing=auto"]));
    assert!(reason.contains("fit_text"), "{reason}");
    let reason = refusal(run(
        &mut app,
        "set_text_note_size",
        &["sizing=fixed_width", "width=-1"],
    ));
    assert!(reason.contains("at or above zero"), "{reason}");
}

#[test]
fn apply_rich_note_edit_sets_the_source_or_applies_the_editor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let answer = answered(run(&mut app, "apply_rich_note_edit", &["source=*new*"]));
    assert_eq!(answer["text"], "*new*");
    assert_eq!(note(&app).text, "*new*");
    undo(&mut app);
    assert_eq!(note(&app).text, SOURCE);

    let reason = refusal(run(&mut app, "apply_rich_note_edit", &[]));
    assert!(reason.contains("no editor is open"), "{reason}");
    answered(run(&mut app, "edit_rich_note", &[]));
    answered(run(
        &mut app,
        "wrap_rich_note_selection",
        &["style=heading"],
    ));
    let applied = answered(run_headless(&mut app, "apply_rich_note_edit", &[]));
    assert_eq!(applied["applied"], true);
    assert!(note(&app).text.starts_with("= "), "{}", note(&app).text);
    assert!(app.ui_state.rich_notes.editor.is_none());
}

#[test]
fn apply_rich_note_edit_refuses_a_plain_note() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plain = TextNote {
        format: TextNoteFormat::Plain,
        ..rich_note(SOURCE)
    };
    let mut app = app_with(dir.path(), plain);
    let reason = refusal(run(&mut app, "apply_rich_note_edit", &["source=x"]));
    assert!(reason.contains("rich text note"), "{reason}");
    assert_eq!(note(&app).text, SOURCE);
}

#[test]
fn edit_rich_note_opens_the_editor_at_the_line_and_column() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let answer = answered(run(&mut app, "edit_rich_note", &["line=2", "column=11"]));
    assert_eq!(answer["editing"], true);
    assert_eq!(answer["source"], SOURCE);
    assert_eq!(answer["cursor"]["line"], 2);
    assert_eq!(answer["cursor"]["byte"], 10);
    let headless = answered(run_headless(&mut app, "edit_rich_note", &[]));
    assert_eq!(headless["cursor"]["line"], 1);
    let reason = refusal(run(&mut app, "edit_rich_note", &["line=0"]));
    assert!(reason.contains("whole number from 1"), "{reason}");
}

#[test]
fn cancel_rich_note_edit_drops_the_typing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    answered(run(&mut app, "edit_rich_note", &[]));
    answered(run(&mut app, "wrap_rich_note_selection", &["style=bold"]));
    let answer = answered(run(&mut app, "cancel_rich_note_edit", &[]));
    assert_eq!(answer["cancelled"], true);
    assert_eq!(note(&app).text, SOURCE);
    let again = answered(run_headless(&mut app, "cancel_rich_note_edit", &[]));
    assert_eq!(again["cancelled"], false, "nothing was open");
}

#[test]
fn wrap_rich_note_selection_needs_the_editor_and_a_style() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let reason = refusal(run(&mut app, "wrap_rich_note_selection", &["style=bold"]));
    assert!(reason.contains("edit_rich_note"), "{reason}");
    answered(run(&mut app, "edit_rich_note", &[]));
    let reason = refusal(run(
        &mut app,
        "wrap_rich_note_selection",
        &["style=underline"],
    ));
    assert!(reason.contains("bold"), "{reason}");
    let answer = answered(run_headless(
        &mut app,
        "wrap_rich_note_selection",
        &["style=list"],
    ));
    assert_eq!(answer["style"], "list");
    assert!(answer["source"].as_str().expect("source").starts_with("- "));
}

#[test]
fn open_rich_note_link_opens_only_a_link_the_note_makes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text = "See #link(\"assets/docs/guide.pdf\")[the guide] and #link(\"../x\")[up].";
    let mut app = app_with(dir.path(), rich_note(text));
    // Nothing is launched from a test.
    app.ui_state.io = crate::io_ports::IoPorts::headless(Default::default());
    let answer = answered(run_headless(
        &mut app,
        "open_rich_note_link",
        &["target=assets/docs/guide.pdf"],
    ));
    assert_eq!(answer["opens"], "file");
    let reason = refusal(run(&mut app, "open_rich_note_link", &["target=other.pdf"]));
    assert!(reason.contains("has no link"), "{reason}");
    let reason = refusal(run(&mut app, "open_rich_note_link", &["target=../x"]));
    assert!(reason.contains("leaves the project folder"), "{reason}");
}

#[test]
fn compile_rich_note_answers_the_size_links_and_problems() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text = "= Notes\nSee #link(\"assets/a.pdf\")[a].";
    let mut app = app_with(dir.path(), rich_note(text));
    let answer = answered(run(&mut app, "compile_rich_note", &[]));
    assert_eq!(answer["compiled"], true, "{answer}");
    assert_eq!(answer["cached"], false);
    assert!(answer["width_mm"].as_f64().expect("width") > 0.0);
    assert_eq!(answer["links"], serde_json::json!(["assets/a.pdf"]));
    let again = answered(run(&mut app, "compile_rich_note", &[]));
    assert_eq!(again["cached"], true, "the cache answers");
    assert_eq!(app.ui_state.rich_notes.compiles, 1);

    let item = signex_types::schematic::SelectedItem::new(
        note_id(),
        signex_types::schematic::SelectedKind::TextNote,
    );
    app.set_rich_note_source(item, "ok\n#nope(").expect("set");
    let broken = answered(run(&mut app, "compile_rich_note", &[]));
    assert_eq!(broken["compiled"], false);
    assert_eq!(broken["problems"][0]["line"], 2, "{broken}");
    assert_eq!(broken["problems"][0]["severity"], "error");
}

/// Without operands (palette, key) each command says what to use instead.
#[test]
fn without_operands_the_commands_name_their_surface() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    for (id, needle) in [
        ("set_text_note_frame", "Frame and Size sections"),
        ("set_text_note_size", "Frame and Size sections"),
        ("edit_rich_note", "double-click the note"),
        ("compile_rich_note", "double-click the note"),
    ] {
        let command = AppCommandId::new(id).expect("id");
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
        let reason = refusal(outcome);
        assert!(reason.contains(needle), "{id}: {reason}");
    }
}
