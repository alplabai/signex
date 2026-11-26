//! #2339: `check_requirements` runs the project's requirements list off the
//! UI thread and answers with a check report: a finding per unmet
//! requirement (`req::<id>`), the met ones in `passed`, compared with a
//! baseline of the same project.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;

use serde_json::{Value, json};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::schematic::Symbol;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::{child, project_app};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

const BOARD: &str = "board.snxpcb";

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn set_requirements(app: &mut Signex, list: Value) {
    app.document_state.projects[0].data.requirements =
        serde_json::from_value(list).expect("a list");
}

/// A board `width` x `height` mm, written to the project folder and listed
/// as the project's board, not opened.
fn list_board(app: &mut Signex, dir: &tempfile::TempDir, width: f64, height: f64) -> PathBuf {
    let mut board: PcbBoard = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = vec![
        Point::new(0.0, 0.0),
        Point::new(width, 0.0),
        Point::new(width, height),
        Point::new(0.0, height),
    ];
    let path = dir.path().join(BOARD);
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(&path, text).expect("write board");
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    path
}

/// A part `reference` of value `value`, placed from library row `row` when
/// given.
fn part(reference: &str, value: &str, row: Option<(Uuid, Uuid)>) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".into(),
        reference: reference.into(),
        value: value.into(),
        unit: 1,
        library_id: row.map(|(library, _)| library),
        row_id: row.map(|(_, row)| row),
        ..Symbol::default()
    }
}

/// The closed child sheet with the parts R1 (from a row of a library that is
/// not open) and R2 (from no row).
fn write_child_with_parts(dir: &tempfile::TempDir) {
    let mut sheet = child();
    let row = (Uuid::from_u128(0xA), Uuid::from_u128(0xB));
    sheet.symbols.push(part("R1", "10k", Some(row)));
    sheet.symbols.push(part("R2", "1k", None));
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("child.snxsch"), text).expect("write sheet");
}

fn check(app: &mut Signex, document: &Path, extra: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("check_requirements").expect("a catalog id");
    let document = format!("document={}", document.display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
    match app.run_query(&id, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn report(app: &mut Signex, document: &Path, extra: &[&str]) -> Value {
    match check(app, document, extra) {
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

fn messages(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| f["message"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_project_without_requirements_answers_with_an_empty_report_that_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    assert_eq!(value["check"], "requirements");
    assert_eq!(value["run"], 1);
    assert_eq!(value["findings"], json!([]));
    assert_eq!(
        value["warnings"],
        json!(["The project lists no requirements"])
    );
    assert_eq!(value["rule_set"]["source"], "project");
}

#[test]
fn board_size_reads_the_closed_board_and_files_the_finding_under_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let board = list_board(&mut app, &dir, 120.0, 50.0);
    set_requirements(
        &mut app,
        json!([
            {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
            {"id": "tall", "kind": "board_size", "max_width_mm": 200, "max_height_mm": 60},
        ]),
    );
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    assert_eq!(
        messages(&value),
        ["The board is 120 x 50 mm, over the limit of 100 x 80 mm"]
    );
    assert_eq!(value["findings"][0]["rule"], "req::size");
    assert_eq!(value["findings"][0]["sheet"], board.display().to_string());
    assert_eq!(value["passed"], json!(["req::tall"]));
}

#[test]
fn a_sheet_or_the_board_of_the_project_names_the_same_check() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let board = list_board(&mut app, &dir, 120.0, 50.0);
    set_requirements(
        &mut app,
        json!([{"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80}]),
    );
    let first = report(&mut app, &dir.path().join("top.snxsch"), &[]);
    let second = report(&mut app, &board, &["baseline=1"]);
    assert_eq!(second["findings"], first["findings"]);
    assert_eq!(second["baseline"], json!({"run": 1, "new": [], "gone": []}));
}

#[test]
fn a_fixed_requirement_is_gone_against_its_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    list_board(&mut app, &dir, 120.0, 50.0);
    set_requirements(
        &mut app,
        json!([{"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80}]),
    );
    let project = project_path(&app);
    let before = report(&mut app, &project, &[]);
    let key = before["findings"][0]["key"].clone();
    list_board(&mut app, &dir, 90.0, 50.0);
    let after = report(&mut app, &project, &["baseline=1"]);
    assert_eq!(after["findings"], json!([]));
    assert_eq!(after["passed"], json!(["req::size"]));
    assert_eq!(after["baseline"]["gone"], json!([key]));
}

#[test]
fn a_baseline_of_another_check_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let top = dir.path().join("top.snxsch");
    let erc_id = AppCommandId::new("check_erc").expect("a catalog id");
    let operand = format!("document={}", top.display());
    let args = CommandArgs::parse_operands(&erc_id, &[operand.as_str()]).expect("operands");
    let (outcome, task) = app.run_query(&erc_id, &args).expect("a query");
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    let (erc, _) = app.settle_deferred(ticket, task);
    assert!(matches!(erc, CommandOutcome::Answered(_)), "{erc:?}");
    let project = project_path(&app);
    let reason = refusal(check(&mut app, &project, &["baseline=1"]));
    assert!(reason.contains("was a `erc` check"), "{reason}");
}

#[test]
fn parts_and_cost_are_checked_on_the_closed_sheets_parts() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_child_with_parts(&dir);
    set_requirements(
        &mut app,
        json!([
            {"id": "parts", "kind": "parts_state"},
            {"id": "cost", "kind": "bom_cost", "quantity": 10, "currency": "USD",
             "max_unit_cost": 5},
        ]),
    );
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    assert_eq!(
        messages(&value),
        [
            "The BOM cost is not known: 2 line(s) have no cached price in USD",
            "R1's library row is in no open library",
            "R2 was not placed from a library row",
        ]
    );
}

/// The messages of the findings filed under `rule`.
fn messages_of(report: &Value, rule: &str) -> Vec<String> {
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == rule)
        .map(|f| f["message"].as_str().unwrap().to_string())
        .collect()
}

/// #2686, headless: a Part lifecycle requirement checks the BOM of the
/// variant it names; with Fitted parts only (the default) a part that
/// variant does not fit is left out, off it is checked; a variant the
/// project does not have is not checked.
#[test]
fn parts_state_checks_the_variant_it_names_and_its_fitted_parts() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let mut sheet = child();
    let row = (Uuid::from_u128(0xA), Uuid::from_u128(0xB));
    sheet.symbols.push(part("R1", "10k", Some(row)));
    let mut r2 = part("R2", "1k", None);
    r2.fields.insert("DNP@Lite".into(), "true".into());
    sheet.symbols.push(r2);
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("child.snxsch"), text).expect("write sheet");
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".into()];
    set_requirements(
        &mut app,
        json!([
            {"id": "base", "kind": "parts_state"},
            {"id": "lite", "kind": "parts_state", "variant": "lite"},
            {"id": "every", "kind": "parts_state", "variant": "Lite", "fitted_only": false},
            {"id": "gone", "kind": "parts_state", "variant": "Gone"},
        ]),
    );
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    let r1 = "R1's library row is in no open library".to_string();
    let r2 = "R2 was not placed from a library row".to_string();
    assert_eq!(messages_of(&value, "req::base"), [r1.clone(), r2.clone()]);
    assert_eq!(messages_of(&value, "req::lite"), [r1.clone()]);
    assert_eq!(messages_of(&value, "req::every"), [r1, r2]);
    assert_eq!(
        messages_of(&value, "req::gone"),
        ["The parts could not be listed: the project has no variant `Gone`"]
    );
}

/// #2686: the catalog command sets the new fields through the tab's path
/// and refuses a variant the project does not have.
#[test]
fn project_set_requirement_sets_variant_and_fitted_parts_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".into()];
    set_requirements(&mut app, json!([{"id": "parts", "kind": "parts_state"}]));
    let project = project_path(&app);
    let set = |app: &mut Signex, field: &str, value: &str| {
        let id = AppCommandId::new("project_set_requirement").expect("a catalog id");
        let operands = [
            format!("document={}", project.display()),
            "requirement=parts".to_string(),
            format!("field={field}"),
            format!("value={value}"),
        ];
        let operands: Vec<&str> = operands.iter().map(String::as_str).collect();
        let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
        app.run_operands(&id, &args).0
    };
    match set(&mut app, "variant", "LITE") {
        CommandOutcome::Answered(value) => {
            assert_eq!(value["requirements"][0]["variant"], "Lite");
        }
        other => panic!("not answered: {other:?}"),
    }
    match set(&mut app, "fitted_only", "false") {
        CommandOutcome::Answered(value) => {
            assert_eq!(value["requirements"][0]["fitted_only"], false);
        }
        other => panic!("not answered: {other:?}"),
    }
    let reason = refusal(set(&mut app, "variant", "Gone"));
    assert!(reason.contains("no variant `Gone`"), "{reason}");
}

#[test]
fn erc_clean_runs_the_projects_erc() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    set_requirements(
        &mut app,
        json!([
            {"id": "erc", "kind": "erc_clean"},
            {"id": "loose", "kind": "erc_clean", "allow_warnings": true},
        ]),
    );
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    // The fixture's ERC has warnings and no error: only the requirement
    // that allows warnings passes.
    assert_eq!(value["passed"], json!(["req::loose"]), "{value}");
    let findings = value["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{value}");
    let evidence = &findings[0]["evidence"];
    assert_eq!(findings[0]["rule"], "req::erc");
    assert_eq!(evidence["check"], "erc");
    assert_eq!(
        (
            &evidence["errors"],
            &evidence["warnings"],
            &evidence["info"]
        ),
        (&json!(0), &json!(4), &json!(2)),
        "{value}"
    );
}

#[test]
fn a_document_outside_every_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let reason = refusal(check(&mut app, &dir.path().join("elsewhere.snxsch"), &[]));
    assert!(
        reason.contains("not an open or referenced document"),
        "{reason}"
    );
}

#[test]
fn drc_clean_runs_the_drc_of_the_projects_board() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    list_board(&mut app, &dir, 50.0, 40.0);
    set_requirements(&mut app, json!([{"id": "drc", "kind": "drc_clean"}]));
    let project = project_path(&app);
    let value = report(&mut app, &project, &[]);
    assert_eq!(value["passed"], json!(["req::drc"]), "{value}");
    assert_eq!(value["findings"], json!([]), "{value}");
}
