//! #3013 — `apply_library_updates`: Library Updates' Update Selected
//! Components (mockup row 135) on the GUI path (`Signex::update` with the
//! dialog's ticks and button) and the headless path (operands), leaving the
//! same state.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_library::RowId;
use signex_types::schematic::{SchematicSheet, StampField};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::library::messages::LibraryMessage;
use crate::library::part_sync::{Choice, ConflictView};
use crate::library::updates_dialog::{
    BumpKind, ConflictChoice, LibraryUpdateEntry, LibraryUpdatesState,
};
use crate::task_driver::TaskDriver;

fn entry(designator: &str, lib_id: &str, bump: BumpKind) -> LibraryUpdateEntry {
    LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: designator.to_string(),
        internal_pn: format!("PN-{designator}"),
        lib_id: lib_id.to_string(),
        library_id: Uuid::nil(),
        library_name: "Lib".to_string(),
        row_id: RowId::from_uuid(Uuid::nil()),
        library_path: PathBuf::from("/nowhere/lib.snxlib"),
        current_version: "1.0.0".to_string(),
        latest_version: "1.0.1".to_string(),
        bump_kind: bump,
        selected: false,
        conflicts: Vec::new(),
    }
}

/// An app with the sheet `a.snxsch` of `dir` open and its updates waiting
/// in the Library Updates dialog: R1 and R2 share one symbol, U1 (a major
/// bump with an MPN conflict) another.
fn app_with_updates(dir: &Path) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let sheet = dir.join("a.snxsch");
    let content = SchematicSheet {
        uuid: Uuid::now_v7(),
        ..Default::default()
    };
    app.open_schematic_tab(sheet.clone(), "a".into(), content);
    let mut u1 = entry("U1", "L:U", BumpKind::Major);
    u1.conflicts.push(ConflictChoice::new(ConflictView {
        field: StampField::Mpn,
        mine: "MINE".into(),
        library: "THEIRS".into(),
    }));
    let entries = vec![
        entry("R1", "L:R", BumpKind::Patch),
        entry("R2", "L:R", BumpKind::Patch),
        u1,
    ];
    let mut state = LibraryUpdatesState::new(sheet.clone(), entries);
    state.engine_instance = app
        .document_state
        .engines
        .get(&sheet)
        .map_or(0, signex_engine::Engine::instance_id);
    app.library.library_updates = Some(state);
    (app, sheet)
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new().settle(app, task).expect("settles");
}

fn send(app: &mut Signex, msg: LibraryMessage) {
    let task = app.update(Message::Library(msg));
    settle(app, task);
}

/// Run `apply_library_updates` with `operands` as a headless run does.
fn headless(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("apply_library_updates").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle(app, task);
    outcome
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

fn uuid_of(app: &Signex, designator: &str) -> Uuid {
    app.library
        .library_updates
        .as_ref()
        .and_then(|state| state.entries.iter().find(|e| e.ref_des == designator))
        .map(|e| e.symbol_uuid)
        .expect("an entry")
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

/// What the button leaves: the dialog closed, and whether the sheet's update
/// is still pending (a part left unticked, or a symbol not read).
fn left(app: &Signex, sheet: &Path) -> (bool, bool) {
    (
        app.library.library_updates.is_none(),
        app.library.skipped_updates_for.contains(sheet),
    )
}

#[test]
fn the_named_parts_update_as_the_dialogs_ticks_and_button_update_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, dialog_sheet) = app_with_updates(dir.path());
    // The dialog: tick U1 and answer its conflict with the library's value.
    let u1 = uuid_of(&by_dialog, "U1");
    send(
        &mut by_dialog,
        LibraryMessage::LibraryUpdatesConflictAnswered {
            symbol_uuid: u1,
            field: StampField::Mpn,
            choice: Choice::UseLibrary,
        },
    );
    send(&mut by_dialog, LibraryMessage::LibraryUpdatesApply);

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_command, command_sheet) = app_with_updates(other.path());
    let outcome = headless(
        &mut by_command,
        &[
            &document(&command_sheet),
            "designators=R1,U1",
            "use_library=U1:mpn",
        ],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    let updating: Vec<&str> = answer["updating"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|part| part["designator"].as_str())
        .collect();
    // R1 and R2 share one embedded symbol, so they update together.
    assert_eq!(updating, ["R1", "R2", "U1"]);
    assert_eq!(
        answer["updating"][2]["use_library"],
        serde_json::json!(["mpn"])
    );
    assert_eq!(answer["updating"][2]["from"], "1.0.0");
    assert_eq!(answer["updating"][2]["to"], "1.0.1");
    assert_eq!(answer["kept"], serde_json::json!([]));
    assert_eq!(
        left(&by_command, &command_sheet),
        left(&by_dialog, &dialog_sheet)
    );
}

/// Without `designators` the dialog's own ticks stand: the patch bumps of R1
/// and R2 are ticked, the major bump of U1 is not.
#[test]
fn without_designators_the_dialogs_default_ticks_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, sheet) = app_with_updates(dir.path());
    let outcome = headless(&mut app, &[&document(&sheet)]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["kept"], serde_json::json!(["U1"]));
    assert_eq!(answer["updating"].as_array().map(Vec::len), Some(2));
    // A part left unticked keeps the sheet's update pending, as from the dialog.
    assert_eq!(left(&app, &sheet), (true, true));
}

/// Another sheet's open dialog waits while the named sheet's queued updates
/// are applied, and is open again afterwards.
#[test]
fn another_sheets_open_dialog_is_open_again_afterwards() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, sheet) = app_with_updates(dir.path());
    let queued = app.library.library_updates.take().expect("waiting");
    let elsewhere = dir.path().join("b.snxsch");
    app.library.library_updates = Some(LibraryUpdatesState::new(
        elsewhere.clone(),
        vec![entry("C1", "L:C", BumpKind::Patch)],
    ));
    app.library.queued_library_updates.push(queued);

    let outcome = headless(&mut app, &[&document(&sheet), "designators=all"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let open = app.library.library_updates.as_ref().expect("open again");
    assert_eq!(open.schematic_path, elsewhere);
    assert!(app.library.queued_library_updates.is_empty());
}

#[test]
fn what_cannot_be_applied_is_refused_and_the_dialog_is_left_as_it_was() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, sheet) = app_with_updates(dir.path());
    let doc = document(&sheet);
    for (operands, needle) in [
        (
            vec![doc.as_str(), "designators=R9"],
            "No library update waits for R9",
        ),
        (
            vec![doc.as_str(), "use_library=U1"],
            "`use_library` takes designator:field",
        ),
        (
            vec![doc.as_str(), "use_library=U1:colour"],
            "U1:colour is not one",
        ),
        (
            vec![doc.as_str(), "use_library=R1:mpn"],
            "R1 has no mpn conflict",
        ),
    ] {
        let reason = refusal(headless(&mut app, &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
        let state = app.library.library_updates.as_ref().expect("still open");
        assert_eq!(state.schematic_path, sheet);
        let ticked: Vec<bool> = state.entries.iter().map(|e| e.selected).collect();
        assert_eq!(ticked, [true, true, false], "{operands:?}");
        assert!(
            state.entries[2]
                .conflicts
                .iter()
                .all(|c| c.choice == Choice::KeepMine)
        );
    }

    app.library.library_updates = None;
    let reason = refusal(headless(&mut app, &[&doc]));
    assert!(reason.contains("No library updates wait for"), "{reason}");
}

/// Without operands a headless run is told the operand it lacks; the palette
/// is told to use the dialog.
#[test]
fn without_operands_it_says_what_it_needs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _sheet) = app_with_updates(dir.path());
    let command = AppCommandId::new("apply_library_updates").expect("a catalog id");
    let (outcome, _) = app.run_headless_command_in_test(&command, CommandArgs::none());
    assert!(
        refusal(outcome).contains("needs the operand `document`"),
        "headless"
    );
    let why = super::operand_handlers::needs_operands_reason(&command).expect("a reason");
    assert!(why.contains("Library Updates dialog"), "{why}");
}
