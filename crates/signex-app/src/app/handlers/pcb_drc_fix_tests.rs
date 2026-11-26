//! #2940: the DRC fixes run from their operands in the window, as the
//! palette and the agent channel run them: one finding or every finding of
//! a rule kind fixed as one undo step that Edit ▸ Undo takes back, a dry run
//! that changes nothing, a locked track left with the reason, and what the
//! check finds afterwards.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::app::command::queries::check_drc_tests::drc_serial;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

const BOARD: &str = "board.snxpcb";
const GND_TRACK: Uuid = Uuid::from_u128(0x2001);
const VCC_TRACK: Uuid = Uuid::from_u128(0x2002);

fn track(id: Uuid, net: u32, y: f64, locked: bool) -> Segment {
    Segment {
        uuid: id,
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked,
    }
}

/// GND and VCC 0.1 mm apart on the Standard setup's 0.15 mm clearance.
fn board(locked: bool) -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "GND"), (2, "VCC")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![
        track(GND_TRACK, 1, 0.0, locked),
        track(VCC_TRACK, 2, 0.35, locked),
    ];
    board
}

/// The board open in its tab, in the project.
fn board_app(dir: &tempfile::TempDir, locked: bool) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    // The built-in rules, whatever the shared test prefs hold.
    app.ui_state.machine_drc_rules = Default::default();
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    let path = dir.path().join(BOARD);
    app.open_pcb_tab(path.clone(), BOARD.into(), board(locked));
    (app, path)
}

fn run(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    app.run_operands_settled(&id, &args).0
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

fn track_y(app: &Signex, path: &Path, id: Uuid) -> f64 {
    let board = app.document_state.pcb_board(path).expect("open");
    board
        .segments
        .iter()
        .find(|s| s.uuid == id)
        .expect("the track")
        .start
        .y
}

fn undo_steps(app: &Signex, path: &Path) -> usize {
    app.document_state
        .board_engine(path)
        .expect("open")
        .undo_len()
}

/// The waiver key of the board's one clearance finding, as `check_drc`
/// names it in `evidence.waiver_key`.
fn clearance_key(app: &Signex, path: &Path) -> String {
    let board = app.document_state.pcb_board(path).expect("open").clone();
    let (rules, _) = app.board_design_rules(path, &board).expect("rules");
    let report = signex_drc::run(&board, &rules);
    let finding = report
        .violations
        .iter()
        .find(|v| v.kind == signex_drc::ViolationKind::Clearance)
        .expect("a clearance finding");
    signex_drc::waiver_key(finding)
}

#[test]
fn fix_drc_moves_a_track_clear_as_one_undo_step() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let got = answer(run(
        &mut app,
        "fix_drc",
        &[&document(&path), "rule=clearance"],
    ));
    assert_eq!(got["changed"], true, "{got}");
    assert_eq!(got["dry_run"], false);
    assert_eq!(got["fixed"].as_array().map(Vec::len), Some(1), "{got}");
    assert_eq!(got["fixed"][0]["rule"], "builtin::clearance");
    assert_eq!(got["fixed"][0]["changed"][0], GND_TRACK.to_string());
    assert_eq!(got["left"], serde_json::json!([]));
    assert_eq!(got["remaining"]["count"], 0, "{got}");
    // 0.05 mm short of 0.15 mm: GND moves 0.051 mm away from VCC.
    assert_eq!(track_y(&app, &path, GND_TRACK), -0.051);
    assert_eq!(track_y(&app, &path, VCC_TRACK), 0.35);
    assert_eq!(undo_steps(&app, &path), 1, "one undo step");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        track_y(&app, &path, GND_TRACK),
        0.0,
        "one undo takes it back"
    );
}

#[test]
fn fix_drc_violation_fixes_the_finding_its_key_names() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let key = clearance_key(&app, &path);
    let violation = format!("violation={key}");
    let got = answer(run(
        &mut app,
        "fix_drc_violation",
        &[&document(&path), &violation],
    ));
    assert_eq!(got["fixed"][0]["id"], key.as_str(), "{got}");
    assert_eq!(got["remaining"]["count"], 0, "{got}");
    // The key names the finding, not where it is: it is gone, not moved.
    let again = run(
        &mut app,
        "fix_drc_violation",
        &[&document(&path), &violation],
    );
    assert!(
        matches!(again, CommandOutcome::Disabled { .. }),
        "{again:?}"
    );
}

#[test]
fn a_dry_run_answers_with_the_fix_and_changes_nothing() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let got = answer(run(
        &mut app,
        "fix_drc",
        &[&document(&path), "rule=clearance", "dry_run=true"],
    ));
    assert_eq!(got["dry_run"], true);
    assert_eq!(got["changed"], false);
    assert_eq!(got["fixed"].as_array().map(Vec::len), Some(1), "{got}");
    assert_eq!(got["remaining"]["count"], 0, "what would remain");
    assert_eq!(track_y(&app, &path, GND_TRACK), 0.0, "nothing moved");
    assert_eq!(undo_steps(&app, &path), 0, "no undo step");
}

#[test]
fn a_finding_between_locked_tracks_is_left_with_the_reason() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, true);
    let got = answer(run(&mut app, "fix_drc", &[&document(&path)]));
    assert_eq!(got["changed"], false);
    assert_eq!(got["fixed"], serde_json::json!([]));
    assert_eq!(got["left"][0]["reason"], "locked", "{got}");
    let why = got["left"][0]["why"].as_str().unwrap_or_default();
    assert!(why.contains(&GND_TRACK.to_string()), "{why}");
    assert_eq!(got["remaining"]["count"], 1, "the finding stays");
    assert_eq!(track_y(&app, &path, GND_TRACK), 0.0);
}

#[test]
fn a_rule_with_no_fix_is_refused_with_the_rules_that_have_one() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let refused = run(&mut app, "fix_drc", &[&document(&path), "rule=short"]);
    let CommandOutcome::Disabled { reason } = refused else {
        panic!("not refused: {refused:?}");
    };
    let reason = signex_i18n::RenderTranslatable::render_en_us(&reason);
    assert!(
        reason.contains("`short` is not a rule with a fix"),
        "{reason}"
    );
    assert!(reason.contains("mask_sliver"), "{reason}");
}

/// The fixes are worked out off the UI thread: the run is deferred, the
/// board is untouched until they land, and the answer comes then.
#[test]
fn fix_drc_is_deferred_and_the_board_changes_only_when_the_fixes_land() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let id = AppCommandId::new("fix_drc").expect("a catalog id");
    let doc = document(&path);
    let args = CommandArgs::parse_operands(&id, &[&doc, "rule=clearance"]).expect("operands");
    let (outcome, task) = app.run_operands(&id, &args);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    assert_eq!(track_y(&app, &path, GND_TRACK), 0.0, "nothing moved yet");
    assert_eq!(undo_steps(&app, &path), 0, "no undo step yet");
    let got = answer(app.settle_deferred(ticket, task).0);
    assert_eq!(got["changed"], true, "{got}");
    assert_eq!(track_y(&app, &path, GND_TRACK), -0.051);
    assert_eq!(undo_steps(&app, &path), 1);
}

/// A board edited while the fixes were worked out refuses them: nothing
/// of the run lands and the edit made meanwhile stays.
#[test]
fn fixes_worked_out_on_a_board_that_moved_on_are_refused() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    let id = AppCommandId::new("fix_drc").expect("a catalog id");
    let doc = document(&path);
    let args = CommandArgs::parse_operands(&id, &[&doc, "rule=clearance"]).expect("operands");
    let (outcome, task) = app.run_operands(&id, &args);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    // The person moves VCC while the fixes are worked out.
    let edit = signex_engine::pcb::BoardCommand::ReplaceItems {
        items: vec![signex_engine::pcb::BoardItem::Segment(track(
            VCC_TRACK, 2, 0.4, false,
        ))],
    };
    app.execute_board_commands(&path, vec![edit], None)
        .expect("the person's edit");
    let landed = app.settle_deferred(ticket, task).0;
    assert!(
        matches!(landed, CommandOutcome::Disabled { .. }),
        "a stale run is refused: {landed:?}"
    );
    assert_eq!(track_y(&app, &path, GND_TRACK), 0.0, "the fix did not land");
    assert_eq!(track_y(&app, &path, VCC_TRACK), 0.4, "the edit stays");
    assert_eq!(undo_steps(&app, &path), 1, "only the person's step");
}

/// #2986: rule text with a syntax error is said in the answer, as
/// `check_drc` and Run DRC say it, not dropped.
#[test]
fn fix_drc_says_the_rule_text_it_did_not_apply() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir, false);
    app.document_state.projects[0].data.rules = Some(signex_types::project::ProjectRules {
        drc: signex_types::project::DrcRuleSet {
            dsl: Some("rule bad on via".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    let got = answer(run(
        &mut app,
        "fix_drc",
        &[&document(&path), "rule=clearance", "dry_run=true"],
    ));
    let warnings = got["warnings"].as_array().expect("warnings");
    assert!(
        warnings
            .iter()
            .any(|w| w == "DRC rules: the rule text was not applied (syntax error)"),
        "{got}"
    );
}

/// #2986: a part whose 3D model could not be read was judged on its stored
/// height: the answer names the model and does not give the remaining
/// count as if the check were whole.
#[test]
fn a_fix_judged_on_a_stored_height_names_the_model_and_gives_no_count() {
    let _serial = drc_serial();
    let dir = tempfile::tempdir().unwrap();
    let (app, path) = board_app(&dir, false);
    let pcb = app.document_state.pcb_board(&path).expect("open").clone();
    let (rules, _) = app.board_design_rules(&path, &pcb).expect("rules");
    let scope = signex_engine::pcb::drc_fix::FixScope::Every(None);
    let run = signex_engine::pcb::drc_fix::fix_findings(
        &pcb,
        &rules,
        &signex_drc::BodyModels::new(),
        &scope,
        &mut signex_types::object_id::UuidV7Ids,
    )
    .expect("fixes");
    let unread =
        "DRC: the 3D model of lib:U1 could not be read (bad file); its stored height is used";
    let worked = super::WorkedFixes {
        run,
        rule_warnings: Vec::new(),
        unread_models: vec![unread.to_string()],
    };
    let got = super::fix_answer(&path, &worked, true, false);
    assert_eq!(got["remaining"]["count"], Value::Null, "{got}");
    assert_eq!(got["remaining"]["complete"], false, "{got}");
    assert_eq!(got["warnings"], serde_json::json!([unread]), "{got}");

    let whole = super::WorkedFixes {
        unread_models: Vec::new(),
        ..worked
    };
    let got = super::fix_answer(&path, &whole, true, false);
    assert_eq!(got["remaining"]["count"], 0, "{got}");
    assert_eq!(got["remaining"]["complete"], true, "{got}");
}
