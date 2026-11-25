//! #1297: `change_component` from operands swaps the placed part `id` names
//! for a library row, as the Change Component dialog's Change does (the same
//! `ChangeComponent` engine command with every Keep now box ticked), as one
//! undo step, and refuses what it cannot change with the reason.

use std::path::PathBuf;

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::part_operand_tests::{Library, library};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::canvas::placement_tests::{grid, sheet_shape};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;

fn sheet_path() -> PathBuf {
    std::env::temp_dir().join("snx-1297").join("swap.snxsch")
}

/// An app with an empty sheet and `lib` mounted, and U1 placed on the sheet
/// from the row `SNX-TWO`.
fn app_with_u1(lib: &Library) -> (Signex, Uuid) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    add_sheet_with(&mut app, &sheet_path(), SchematicSheet::default());
    app.library
        .open_library(lib.snxlib.clone())
        .expect("mount library");
    let placed = run(
        &mut app,
        "place_component",
        &[
            format!("library={}", lib.id),
            "part=SNX-TWO".into(),
            format!("at={},{}", grid(8.0, 16.0).x, grid(8.0, 16.0).y),
        ],
    );
    assert!(matches!(placed, CommandOutcome::Answered(_)), "{placed:?}");
    let id = symbols(&app)[0].uuid;
    (app, id)
}

fn run(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &refs).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn change(app: &mut Signex, lib: &Library, id: &str, part: &str) -> CommandOutcome {
    let operands = [
        format!("document={}", sheet_path().display()),
        format!("id={id}"),
        format!("library={}", lib.id),
        format!("part={part}"),
    ];
    run(app, "change_component", &operands)
}

fn symbols(app: &Signex) -> &[signex_types::schematic::Symbol] {
    &app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
}

fn row_b(app: &Signex) -> Uuid {
    app.library.open_libraries[0]
        .tables
        .values()
        .flatten()
        .find(|row| row.internal_pn.as_str() == "SNX-TWO-B")
        .expect("row B")
        .row_id
}

#[test]
fn the_operands_swap_the_part_as_the_dialogs_change_does_in_one_undo_step() {
    let lib = library();
    let (mut by_operands, id) = app_with_u1(&lib);
    let before = sheet_shape(&by_operands);
    let answer = match change(&mut by_operands, &lib, &id.to_string(), "SNX-TWO-B") {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    };
    assert_eq!(answer["designator"], "U1");
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["to"]["internal_pn"], "SNX-TWO-B");
    assert_eq!(
        answer["to"]["part"],
        Value::from(row_b(&by_operands).to_string())
    );
    assert_eq!(symbols(&by_operands)[0].row_id, Some(row_b(&by_operands)));

    // The dialog's Change: the same engine command on the same sheet.
    let (mut by_dialog, dialog_id) = app_with_u1(&lib);
    let open: Vec<(Uuid, PathBuf)> = by_dialog
        .library
        .open_libraries
        .iter()
        .map(|l| (l.library_id, l.root.clone()))
        .collect();
    let resolved = crate::library::place::resolve_placement(
        &lib.snxlib,
        "ICs",
        signex_library::RowId(row_b(&by_dialog)),
        &open,
    )
    .expect("resolve");
    let replacement = crate::app::handlers::placed_part::replacement_of(&resolved);
    let doc = by_dialog.document_state.active_engine().unwrap().document();
    let kept = replacement.user_parameters(doc, dialog_id);
    let command = signex_engine::Command::ChangeComponent {
        symbol_id: dialog_id,
        replacement: replacement.keeping_parameters(doc, dialog_id, &kept),
    };
    assert!(by_dialog.apply_engine_commands(vec![command], true, true));
    assert_eq!(sheet_shape(&by_operands), sheet_shape(&by_dialog));

    // One undo step takes it back.
    let _ = by_operands.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
    assert_eq!(sheet_shape(&by_operands), before);
}

#[test]
fn what_cannot_be_changed_is_refused_with_the_reason() {
    let lib = library();
    let (mut app, id) = app_with_u1(&lib);
    let before = sheet_shape(&app);
    let refusal = |outcome: CommandOutcome| match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    };
    let reason = refusal(change(&mut app, &lib, "U1", "SNX-TWO-B"));
    assert!(
        reason.contains("`id` is the uuid of a placed part"),
        "{reason}"
    );
    let reason = refusal(change(
        &mut app,
        &lib,
        &Uuid::nil().to_string(),
        "SNX-TWO-B",
    ));
    assert!(reason.contains("no placed part with id"), "{reason}");
    let reason = refusal(change(&mut app, &lib, &id.to_string(), "NO-SUCH"));
    assert!(reason.contains("has no part `NO-SUCH`"), "{reason}");
    assert_eq!(sheet_shape(&app), before, "a refusal changes nothing");
}
