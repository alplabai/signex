//! #2939: `query_part_heights` answers each part's designator, side and
//! height and where the height came from, the same in the window and in a
//! headless run; it names only an open board and changes nothing.

use serde_json::{Value, json};
use signex_drc_dsl::fixture::BaseRule;
use signex_i18n::RenderTranslatable;

use super::project_name_tests::project_app;
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::keymap::AppCommandId;

/// An app with a two-part board open: U1 stored 3 mm tall on top, U2 with
/// no height on the bottom.
fn board_app(dir: &tempfile::TempDir) -> (Signex, std::path::PathBuf) {
    let mut app = project_app(dir);
    let mut board = BaseRule::Courtyard.board(false);
    board.footprints[0].component.height_mm = 3.0;
    board.footprints[1].layer = "BottomCopper".into();
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board.snxpcb".into(), board);
    (app, path)
}

fn args(path: &std::path::Path) -> (AppCommandId, CommandArgs) {
    let command = AppCommandId::new("query_part_heights").expect("a catalog id");
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&command, &[document.as_str()]).expect("parse");
    (command, args)
}

/// Run to its answer, its model read included.
fn settled(app: &mut Signex, ran: (CommandOutcome, iced::Task<crate::app::Message>)) -> Value {
    let outcome = match ran {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    };
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn each_part_has_its_side_height_and_source_in_both_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let before = app.document_state.pcb_revisions.get(&path).copied();
    let (command, parsed) = args(&path);
    let ran = app.run_operands(&command, &parsed);
    let window = settled(&mut app, ran);
    let ran = app.run_headless_command_in_test(&command, parsed);
    let headless = settled(&mut app, ran);
    assert_eq!(window, headless);
    assert_eq!(
        window["parts"],
        json!([
            {
                "id": "00000000-0000-0000-0000-000000001001",
                "designator": "U1",
                "side": "top",
                "height_mm": 3.0,
                "bottom_mm": 0.0,
                "source": "stored",
                "problem": null,
            },
            {
                "id": "00000000-0000-0000-0000-000000001002",
                "designator": "U2",
                "side": "bottom",
                "height_mm": null,
                "bottom_mm": null,
                "source": "none",
                "problem": null,
            },
        ])
    );
    assert_eq!(window["unread"], json!([]));
    let after = app.document_state.pcb_revisions.get(&path).copied();
    assert_eq!(after, before, "nothing changed");
}

#[test]
fn a_board_that_is_not_open_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let path = dir.path().join("closed.snxpcb");
    let (command, parsed) = args(&path);
    let (outcome, _) = app.run_operands(&command, &parsed);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(!reason.render_en_us().is_empty());
}
