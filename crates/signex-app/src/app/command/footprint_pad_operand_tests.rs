//! #3013 — `footprint_move_pads` and `footprint_align_pads`: the edits of
//! Move Selection By and Align Selection, on the GUI path (`Signex::update`
//! with the dialog's messages) and the headless path (operands), each one
//! undo step of the footprint that leaves the same pads.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;

use super::{CommandArgs, CommandOutcome};
use crate::app::{FootprintDocument, Message, Signex};
use crate::keymap::AppCommandId;
use crate::library::editor::footprint::{AlignOp, FootprintEditorState};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Pads 1, 2 and 3 at (0, 0), (4, 6) and (10, 2).
const PADS: [(f64, f64); 3] = [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)];

/// An app with the footprint `f.snxfpt` in `dir` open in its tab, holding
/// pads at `positions` (numbered 1, 2, ...), clean, with nothing selected.
fn app_with_footprint(dir: &Path, positions: &[(f64, f64)]) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = dir.join("f.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("F"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    for &(x, y) in positions {
        editor.state.add_pad_at(x, y);
    }
    editor.with_parts(|state, primitive| {
        FootprintEditorState::sync_pads_to_primitive(state, primitive);
    });
    // `add_pad_at` selects the pad it adds; the fixture starts with none.
    editor.state.selected_pad = None;
    editor.state.selected_pads_extra.clear();
    editor.set_dirty(false);
    editor.history.undo_steps_mut().clear();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "f".into(),
        path: path.clone(),
        project_id: None,
        kind: crate::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path)
}

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a FootprintDocument {
    app.document_state
        .footprint_editors
        .get(path)
        .expect("the footprint is open")
}

fn centres(app: &Signex, path: &Path) -> Vec<(f64, f64)> {
    editor(app, path)
        .state
        .pads
        .iter()
        .map(|pad| pad.position_mm)
        .collect()
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

/// Run `id` with `operands` as a headless run does.
fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

/// Run `id` from its operands through `run_operands`, as a CLI run attached
/// to a window does.
fn operands(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_operands_settled(&command, &args).0
}

fn answer(outcome: CommandOutcome) -> serde_json::Value {
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

/// What a dialog's button sends to the footprint at `path`.
fn send(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

fn select_all(app: &mut Signex, path: &Path) {
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(path)
        .expect("the footprint is open");
    editor.state.selected_pad = Some(0);
    editor.state.selected_pads_extra = (1..editor.state.pads.len()).collect();
}

#[test]
fn move_pads_from_operands_is_move_selection_by_on_those_pads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, dialog_path) = app_with_footprint(dir.path(), &PADS);
    select_all(&mut by_dialog, &dialog_path);
    send(&mut by_dialog, &dialog_path, FootprintEditorMsg::MoveByOpen);
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::MoveBySetX("1.5".into()),
    );
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::MoveBySetY("-0.5".into()),
    );
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::MoveByConfirm,
    );

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_command, path) = app_with_footprint(other.path(), &PADS);
    let doc = document(&path);
    let answered = answer(headless(
        &mut by_command,
        "footprint_move_pads",
        &[&doc, "pad_numbers=1,2,3", "by=1.5,-0.5"],
    ));

    assert_eq!(
        centres(&by_command, &path),
        centres(&by_dialog, &dialog_path)
    );
    assert_eq!(
        centres(&by_command, &path),
        [(1.5, -0.5), (5.5, 5.5), (11.5, 1.5)]
    );
    assert_eq!(answered["changed"], true);
    assert_eq!(answered["pads"][0]["number"], "1");
    assert_eq!(answered["pads"][2]["x"], 11.5);
    assert_eq!(answered["document"], path.display().to_string());
    let edited = editor(&by_command, &path);
    assert_eq!(edited.history.undo_len(), 1, "one undo step");
    assert!(edited.is_dirty());
    assert_eq!(
        edited.state.selected_pad, None,
        "the selection is not touched"
    );
    assert_eq!(editor(&by_dialog, &dialog_path).history.undo_len(), 1);
}

#[test]
fn move_pads_moves_only_the_pads_it_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_footprint(dir.path(), &PADS);
    let doc = document(&path);
    let answered = answer(operands(
        &mut app,
        "footprint_move_pads",
        &[&doc, "pad_numbers=3, 3", "by=0,2"],
    ));
    assert_eq!(centres(&app, &path), [(0.0, 0.0), (4.0, 6.0), (10.0, 4.0)]);
    assert_eq!(answered["pads"].as_array().map(Vec::len), Some(1));
}

#[test]
fn align_pads_from_operands_is_align_selection_on_those_pads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, dialog_path) = app_with_footprint(dir.path(), &PADS);
    select_all(&mut by_dialog, &dialog_path);
    send(&mut by_dialog, &dialog_path, FootprintEditorMsg::AlignOpen);
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::AlignSetHorizontal(Some(AlignOp::DistributeH)),
    );
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::AlignSetVertical(Some(AlignOp::Top)),
    );
    send(
        &mut by_dialog,
        &dialog_path,
        FootprintEditorMsg::AlignConfirm,
    );

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_command, path) = app_with_footprint(other.path(), &PADS);
    let doc = document(&path);
    let answered = answer(headless(
        &mut by_command,
        "footprint_align_pads",
        &[
            &doc,
            "pad_numbers=1,2,3",
            "horizontal=space",
            "vertical=top",
        ],
    ));

    assert_eq!(
        centres(&by_command, &path),
        centres(&by_dialog, &dialog_path)
    );
    assert_eq!(
        centres(&by_command, &path),
        [(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]
    );
    assert_eq!(answered["changed"], true);
    assert_eq!(answered["skipped"], serde_json::json!([]));
    assert_eq!(editor(&by_command, &path).history.undo_len(), 1);
    assert_eq!(editor(&by_dialog, &dialog_path).history.undo_len(), 1);
}

/// Two pads: Space needs three, so the horizontal axis is skipped and named,
/// and the vertical one still runs; one pad aligns nothing and is refused.
#[test]
fn an_axis_the_pads_are_too_few_for_is_skipped_or_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_footprint(dir.path(), &PADS);
    let doc = document(&path);
    let answered = answer(operands(
        &mut app,
        "footprint_align_pads",
        &[
            &doc,
            "pad_numbers=1,2",
            "horizontal=space",
            "vertical=bottom",
        ],
    ));
    assert_eq!(answered["skipped"], serde_json::json!(["horizontal"]));
    assert_eq!(centres(&app, &path), [(0.0, 6.0), (4.0, 6.0), (10.0, 2.0)]);

    let reason = refusal(operands(
        &mut app,
        "footprint_align_pads",
        &[&doc, "pad_numbers=3", "horizontal=left"],
    ));
    assert!(reason.contains("aligning needs 2"), "{reason}");
    assert_eq!(editor(&app, &path).history.undo_len(), 1);
}

#[test]
fn what_cannot_be_done_is_refused_with_nothing_moved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_footprint(dir.path(), &PADS);
    // Two pads numbered 2: neither can be named.
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .expect("open")
        .state
        .pads[2]
        .number = "2".into();
    let doc = document(&path);
    let elsewhere = format!("document={}", dir.path().join("x.snxsch").display());
    for (id, args, needle) in [
        (
            "footprint_move_pads",
            vec![doc.as_str(), "pad_numbers=9", "by=1,1"],
            "no pad `9`",
        ),
        (
            "footprint_move_pads",
            vec![doc.as_str(), "pad_numbers=2", "by=1,1"],
            "2 pads are numbered `2`",
        ),
        (
            "footprint_move_pads",
            vec![doc.as_str(), "pad_numbers=1,", "by=1,1"],
            "comma separated",
        ),
        (
            "footprint_move_pads",
            vec![elsewhere.as_str(), "pad_numbers=1", "by=1,1"],
            "x.snxsch",
        ),
        (
            "footprint_align_pads",
            vec![doc.as_str(), "pad_numbers=1", "horizontal=up"],
            "`horizontal` is one of",
        ),
        (
            "footprint_align_pads",
            vec![doc.as_str(), "pad_numbers=1", "vertical=none"],
            "other than `none`",
        ),
    ] {
        let reason = refusal(operands(&mut app, id, &args));
        assert!(reason.contains(needle), "{id} {args:?}: {reason}");
    }
    assert_eq!(centres(&app, &path), [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)]);
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
    assert!(!editor(&app, &path).is_dirty());
}

/// Without operands a headless run is told the operand it lacks; the palette
/// is told to use the dialogs.
#[test]
fn without_operands_each_says_what_it_needs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _path) = app_with_footprint(dir.path(), &PADS);
    for id in ["footprint_move_pads", "footprint_align_pads"] {
        let command = AppCommandId::new(id).expect("a catalog id");
        // No operands at all, as a bare headless run sends: the parse would
        // refuse a missing required operand before the handler is asked.
        let (outcome, _) = app.run_headless_command_in_test(&command, CommandArgs::none());
        let reason = refusal(outcome);
        assert!(
            reason.contains("needs the operand `document`"),
            "{id}: {reason}"
        );
        let why = super::operand_handlers::needs_operands_reason(&command).expect("a reason");
        assert!(why.contains("Move Selection By or Align Selection"), "{id}");
    }
}

/// #3013: a `.snxfpt` opened by `open_document` in a headless run, with no
/// window and no editor put in by hand, is a document the pad commands name.
#[test]
fn a_footprint_opened_headless_takes_the_pad_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("soic.snxfpt");
    // The stock SOIC-8, copied out of the stock library so it opens as itself.
    let stock = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/stock-library/footprints/019df45d-af5f-7bb0-914a-26b538180e51.snxfpt");
    std::fs::copy(stock, &path).expect("copy the footprint");
    let (mut app, _task) = Signex::new();

    let open = format!("path={}", path.display());
    let opened = answer(headless(&mut app, "open_document", &[&open]));
    assert_eq!(opened["kind"], "footprint");
    assert_eq!(opened["path"], path.display().to_string());

    let doc = document(&path);
    let before = answer(headless(
        &mut app,
        "footprint_move_pads",
        &[&doc, "pad_numbers=1,2", "by=0,0"],
    ));
    assert_eq!(before["changed"], false);
    let after = answer(headless(
        &mut app,
        "footprint_move_pads",
        &[&doc, "pad_numbers=1,2", "by=1,0"],
    ));
    assert_eq!(after["changed"], true);
    for pad in 0..2 {
        let x = |answer: &serde_json::Value| answer["pads"][pad]["x"].as_f64().expect("x");
        let y = |answer: &serde_json::Value| answer["pads"][pad]["y"].as_f64().expect("y");
        assert!((x(&after) - x(&before) - 1.0).abs() < 1e-9, "pad {pad}");
        assert_eq!(y(&after), y(&before), "pad {pad}");
    }
    assert_eq!(editor(&app, &path).history.undo_len(), 1);
    assert!(editor(&app, &path).is_dirty());
}
