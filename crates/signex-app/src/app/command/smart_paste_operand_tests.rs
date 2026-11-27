//! `smart_paste` from its operands (#2665, mockup row 155): a CLI run pastes
//! once at an offset or as an array, with the Smart Paste dialog's numbering
//! and net names, as one undo step; the dialog and its click place the same.

use std::path::Path;

use signex_i18n::RenderTranslatable;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::handlers::paste::at_once_tests::{
    app_with_copy, copy_everything, landed, sheet_to_copy,
};
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::smart_paste::{PlaceMode, SmartPasteMsg, SmartPasteState};

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("smart_paste").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_operands_settled(&command, &args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
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

fn undo_position(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .expect("a sheet")
        .undo_position()
}

#[test]
fn an_array_lands_as_one_undo_step_and_answers_what_landed() {
    let mut app = app_with_copy(sheet_to_copy());
    let before = landed(&app);
    let steps = undo_position(&app);
    let answer = answered(run(
        &mut app,
        &["copies=3", "delta=10.16,0", "at=50.8,50.8"],
    ));
    assert_eq!(answer["copies"], 3);
    assert_eq!(answer["at"], serde_json::json!([50.8, 50.8]));
    assert_eq!(answer["delta"], serde_json::json!([10.16, 0.0]));
    assert_eq!(
        answer["designators"],
        serde_json::json!([{ "from": "R1", "to": ["R2", "R3", "R4"] }])
    );
    assert_eq!(
        answer["nets"],
        serde_json::json!([
            { "net": "D0", "after": ["D0_2", "D0_3", "D0_4"] },
            { "net": "GND", "after": "power" },
        ])
    );
    assert_eq!(answer["items"], 9, "three items per copy");
    assert_eq!(undo_position(&app), steps + 1, "one undo step");
    app.document_state
        .active_engine_mut()
        .expect("a sheet")
        .undo()
        .expect("undo");
    assert_eq!(landed(&app), before, "one undo takes every copy back");
}

/// Click equivalence: the dialog's Array with its click at a point places
/// what the operands with `at` at that point place.
#[test]
fn the_operands_place_what_the_dialog_and_its_click_place() {
    let mut gui = app_with_copy(sheet_to_copy());
    gui.open_smart_paste();
    for msg in [
        SmartPasteMsg::ModePicked(PlaceMode::Array),
        SmartPasteMsg::CopiesEdited("2".into()),
        SmartPasteMsg::StepXEdited("10.16".into()),
        SmartPasteMsg::StepYEdited("0".into()),
        SmartPasteMsg::PasteConfirmed,
    ] {
        let _ = gui.update(Message::Edit(EditMsg::SmartPasteDialog(msg)));
    }
    assert!(gui.interaction_state.placement.is_some(), "on the cursor");
    let _ = gui.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: 60.96,
        world_y: 60.96,
    }));

    let mut cli = app_with_copy(sheet_to_copy());
    answered(run(
        &mut cli,
        &["copies=2", "delta=10.16,0", "at=60.96,60.96"],
    ));
    assert_eq!(landed(&cli), landed(&gui));
}

/// A choice left out is the dialog's: one copy, the dialog's offset, Next
/// free, the nets rewritten with `${Net}_${n}` from 2.
#[test]
fn left_out_choices_are_the_dialogs_defaults() {
    let mut app = app_with_copy(sheet_to_copy());
    let fragment = app
        .interaction_state
        .clipboard
        .as_ref()
        .expect("copy")
        .payload
        .fragment()
        .clone();
    // The dialog opens with this offset, held to two places in its field.
    let dialog = SmartPasteState::new("x.snxsch".into(), app.smart_paste_step_mm(&fragment));
    let offset_x: f64 = dialog.offset_x.parse().expect("a number");
    let answer = answered(run(&mut app, &[]));
    assert_eq!(answer["copies"], 1);
    assert_eq!(answer["delta"], serde_json::json!([offset_x, 0.0]));
    assert_eq!(answer["designators"][0]["to"], serde_json::json!(["R2"]));
    assert_eq!(answer["nets"][0]["after"], serde_json::json!(["D0_2"]));
}

#[test]
fn keep_and_keep_if_free_are_the_dialogs_other_choices() {
    let mut app = app_with_copy(sheet_to_copy());
    let answer = answered(run(&mut app, &["nets=keep", "designators=keep_if_free"]));
    assert_eq!(answer["nets"][0]["after"], "joins");
    // R1 is still on the sheet, so Keep unless taken numbers on.
    assert_eq!(answer["designators"][0]["to"], serde_json::json!(["R2"]));
}

/// A choice that does not read is refused naming its operand, and nothing
/// is placed.
#[test]
fn a_choice_that_does_not_read_is_refused_naming_its_operand() {
    let mut app = app_with_copy(sheet_to_copy());
    let before = landed(&app);
    for (operands, named) in [
        (&["copies=0"][..], "`copies`"),
        (&["copies=x"][..], "`copies`"),
        (&["delta=5"][..], "`delta`"),
        (&["delta=a,b"][..], "`delta`"),
        (&["designators=reset"][..], "`designators`"),
        (&["nets=join"][..], "`nets`"),
        (&["net_start=two"][..], "`net_start`"),
        (&["copies=2", "net_pattern=X"][..], "`net_pattern`"),
        (&["at=R1.1"][..], "`at`"),
    ] {
        let reason = refusal(run(&mut app, operands));
        assert!(reason.contains(named), "{operands:?}: {reason}");
    }
    assert_eq!(landed(&app), before);
}

#[test]
fn with_nothing_copied_it_is_refused() {
    let mut app = app_with_copy(sheet_to_copy());
    app.interaction_state.clipboard = None;
    let outcome = run(&mut app, &["copies=2"]);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
}

fn write(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// A closed project sheet is read before the paste runs: its D0_2 stops the
/// default rewrite, and with another Start its R7 numbers the copy R8.
#[test]
fn a_closed_project_sheet_is_read_first_and_counts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut child = SchematicSheet::default();
    child.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".into(),
        reference: "R7".into(),
        position: Point::new(10.16, 10.16),
        unit: 1,
        ..Symbol::default()
    });
    child
        .labels
        .push(Label::new("D0_2", Point::new(20.32, 10.16), LabelType::Net));
    write(&dir.path().join("top.snxsch"), sheet_to_copy());
    write(&dir.path().join("child.snxsch"), child);
    let mut app = app_workspace(
        &dir.path().to_string_lossy(),
        &["top.snxsch", "child.snxsch"],
    );
    app.open_or_focus_sheet_path(dir.path().join("top.snxsch"));
    copy_everything(&mut app);
    let command = AppCommandId::new("smart_paste").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &["copies=1"]).expect("operands");
    let (outcome, task) = app.run_operands(&command, &args);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    let before = landed(&app);
    let reason = refusal(app.settle_deferred(ticket, task).0);
    assert!(reason.contains("D0_2 already exists"), "{reason}");
    assert_eq!(landed(&app), before, "nothing placed");
    let answer = answered(run(&mut app, &["copies=1", "net_start=3"]));
    assert_eq!(answer["designators"][0]["to"], serde_json::json!(["R8"]));
    assert_eq!(answer["nets"][0]["after"], serde_json::json!(["D0_3"]));
}
