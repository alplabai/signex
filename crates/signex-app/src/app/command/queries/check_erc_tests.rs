//! #2199 step D (decision comment 6798772157): `check_erc` runs ERC on the
//! project of a named sheet off the UI thread and answers with a check
//! report; it leaves the ERC panel, the ERC state and Messages alone.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_types::format::SnxSchematic;
use signex_types::project::{ErcWaiver, ProjectRules};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::{child, parent, project_app};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn top(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("top.snxsch")
}

fn child_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("child.snxsch")
}

fn write(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// The child sheet with a free wire from `(x, 100)` to `(x + 10.16, 100)`.
fn child_with_free_wire(x: f64) -> SchematicSheet {
    with_free_wire(&child(), x)
}

/// `base` with a new free wire from `(x, 100)` to `(x + 10.16, 100)`.
fn with_free_wire(base: &SchematicSheet, x: f64) -> SchematicSheet {
    let mut sheet = base.clone();
    sheet.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(x, 100.0),
        end: Point::new(x + 10.16, 100.0),
        ..Default::default()
    });
    sheet
}

/// Start `check_erc` on `sheet`; its outcome and the run's task.
fn start(
    app: &mut Signex,
    sheet: &Path,
    extra: &[&str],
) -> (CommandOutcome, iced::Task<crate::app::Message>) {
    let id = AppCommandId::new("check_erc").expect("a catalog id");
    let document = format!("document={}", sheet.display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
    app.run_query(&id, &args).expect("a query")
}

/// Run `check_erc` to its end.
fn check(app: &mut Signex, sheet: &Path, extra: &[&str]) -> CommandOutcome {
    match start(app, sheet, extra) {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn report(app: &mut Signex, sheet: &Path, extra: &[&str]) -> Value {
    match check(app, sheet, extra) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn keys(report: &Value, list: &str) -> Vec<String> {
    report[list]
        .as_array()
        .expect("a list")
        .iter()
        .map(|f| f["key"].as_str().expect("a key").to_string())
        .collect()
}

fn on_sheet(report: &Value, sheet: &Path) -> Vec<Value> {
    let name = sheet.display().to_string();
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["sheet"] == name.as_str())
        .cloned()
        .collect()
}

#[test]
fn a_check_answers_with_the_report_of_the_whole_project_its_closed_sheet_included() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let (outcome, task) = start(&mut app, &top(&dir), &[]);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    let CommandOutcome::Answered(report) = app.settle_deferred(ticket, task).0 else {
        panic!("not answered");
    };
    assert_eq!(report["check"], "erc", "{report}");
    assert_eq!(report["run"], 1, "{report}");
    assert!(!on_sheet(&report, &top(&dir)).is_empty(), "{report}");
    // The child is closed: its findings come from the file read by the run.
    assert!(!on_sheet(&report, &child_path(&dir)).is_empty(), "{report}");
    let first = &report["findings"][0];
    for field in [
        "rule", "severity", "message", "evidence", "items", "sheet", "key",
    ] {
        assert!(first.get(field).is_some(), "{field} missing: {first}");
    }
    assert!(report["waived"].as_array().unwrap().is_empty());
    assert!(report.get("baseline").is_none(), "{report}");
}

#[test]
fn a_closed_sheet_is_read_when_the_run_lands_not_when_it_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let before = on_sheet(&report(&mut app, &top(&dir), &[]), &child_path(&dir)).len();
    let (CommandOutcome::Deferred { ticket }, task) = start(&mut app, &top(&dir), &[]) else {
        panic!("not deferred");
    };
    // Written after `update` returned: only a read in the run can see it.
    write(&child_path(&dir), child_with_free_wire(100.0));
    let CommandOutcome::Answered(report) = app.settle_deferred(ticket, task).0 else {
        panic!("not answered");
    };
    let after = on_sheet(&report, &child_path(&dir));
    assert!(
        after.len() > before,
        "{before} -> {}: {report}",
        after.len()
    );
    assert!(
        after.iter().any(|f| f["evidence"]["at"][0] == 100.0),
        "{report}"
    );
}

#[test]
fn the_same_project_checked_twice_gives_the_same_findings_in_the_same_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let first = report(&mut app, &top(&dir), &[]);
    let second = report(&mut app, &top(&dir), &[]);
    assert_eq!(second["run"], 2);
    assert_eq!(first["findings"], second["findings"]);
    assert_eq!(
        serde_json::to_string(&first["findings"]).unwrap(),
        serde_json::to_string(&second["findings"]).unwrap()
    );
}

#[test]
fn a_finding_keeps_its_key_when_its_wire_moves() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let wired = child_with_free_wire(100.0);
    write(&child_path(&dir), wired.clone());
    let first = report(&mut app, &top(&dir), &[]);
    let mut moved = wired;
    let free = moved.wires.last_mut().expect("the free wire");
    free.start = Point::new(150.0, 120.0);
    free.end = Point::new(160.16, 120.0);
    write(&child_path(&dir), moved);
    let second = report(&mut app, &top(&dir), &[]);
    assert_eq!(keys(&first, "findings"), keys(&second, "findings"));
    assert_ne!(first["findings"], second["findings"], "the evidence moved");
}

#[test]
fn a_baseline_lists_the_new_and_the_gone_findings() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    // One child, so its own objects keep their ids from run to run.
    let base = child();
    write(&child_path(&dir), base.clone());
    let first = report(&mut app, &top(&dir), &[]);
    write(&child_path(&dir), with_free_wire(&base, 100.0));
    let second = report(&mut app, &top(&dir), &["baseline=1"]);
    let added: Vec<String> = keys(&second, "findings")
        .into_iter()
        .filter(|k| !keys(&first, "findings").contains(k))
        .collect();
    assert!(!added.is_empty(), "{second}");
    assert_eq!(second["baseline"]["run"], 1, "{second}");
    assert_eq!(second["baseline"]["new"], serde_json::json!(added));
    assert_eq!(second["baseline"]["gone"], serde_json::json!([]));

    // The free wire goes, another comes: against run 2, each side is listed.
    write(&child_path(&dir), with_free_wire(&base, 200.0));
    let third = report(&mut app, &top(&dir), &["baseline=2"]);
    let gone: Vec<String> = third["baseline"]["gone"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().to_string())
        .collect();
    assert_eq!(gone, added, "{third}");
    let new = third["baseline"]["new"].as_array().unwrap();
    assert_eq!(new.len(), added.len(), "{third}");
    assert!(
        new.iter()
            .all(|k| !added.contains(&k.as_str().unwrap().to_string()))
    );
}

#[test]
fn a_baseline_that_is_not_kept_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let why = refusal(check(&mut app, &top(&dir), &["baseline=7"]));
    assert!(why.contains("no such run"), "{why}");
    // The refused run is not numbered.
    assert_eq!(report(&mut app, &top(&dir), &[])["run"], 1);
    for _ in 0..crate::app::state::check_runs::KEPT_CHECK_RUNS {
        report(&mut app, &top(&dir), &[]);
    }
    let why = refusal(check(&mut app, &top(&dir), &["baseline=1"]));
    assert!(why.contains("no longer kept"), "{why}");
    let (outcome, _) = start(&mut app, &top(&dir), &["baseline=latest"]);
    assert!(refusal(outcome).contains("run number"));
}

#[test]
fn a_baseline_of_another_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    report(&mut app, &top(&dir), &[]);
    let loose = dir.path().join("elsewhere").join("loose.snxsch");
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &loose,
        SchematicSheet::default(),
    );
    let why = refusal(check(&mut app, &loose, &["baseline=1"]));
    assert!(why.contains("not this one"), "{why}");
    // On its own, the loose sheet is checked alone.
    let alone = report(&mut app, &loose, &[]);
    assert!(
        alone["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["sheet"] == loose.display().to_string().as_str()),
        "{alone}"
    );
}

/// A per-finding waiver of `finding`, as the ERC panel would store it, with
/// the fingerprint `fingerprint` (the finding's own, or a stale one).
fn waiver_of(finding: &Value, fingerprint: Option<&str>) -> ErcWaiver {
    let key = &finding["evidence"]["waiver_key"];
    let objects = key["objects"]
        .as_array()
        .expect("a waivable finding")
        .iter()
        .map(|o| o.as_str().unwrap().to_string())
        .collect();
    ErcWaiver {
        rule_id: key["rule_id"].as_str().unwrap().to_string(),
        objects,
        fingerprint: fingerprint
            .unwrap_or_else(|| key["fingerprint"].as_str().unwrap())
            .to_string(),
        reason: "checked by hand".into(),
        author: "hg".into(),
        date: "2026-11-11".into(),
        message: finding["message"].as_str().unwrap().to_string(),
        unknown: Default::default(),
    }
}

fn set_rules(app: &mut Signex, rules: ProjectRules) {
    app.document_state.projects[0].data.rules = Some(rules);
}

#[test]
fn a_waived_finding_is_listed_apart_with_its_waiver_and_a_lapsed_waiver_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let first = report(&mut app, &top(&dir), &[]);
    let wires: Vec<&Value> = first["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "builtin::dangling_wire_endpoint")
        .collect();
    let (waived, lapsing) = (wires[0], wires[1]);
    set_rules(
        &mut app,
        ProjectRules {
            erc_waivers: vec![waiver_of(waived, None), waiver_of(lapsing, Some("stale"))],
            ..ProjectRules::default()
        },
    );
    let second = report(&mut app, &top(&dir), &[]);
    let found = keys(&second, "findings");
    assert!(
        !found.contains(&waived["key"].as_str().unwrap().to_string()),
        "{second}"
    );
    assert_eq!(second["waived"].as_array().unwrap().len(), 1, "{second}");
    let entry = &second["waived"][0];
    assert_eq!(entry["finding"]["key"], waived["key"], "{second}");
    assert_eq!(entry["waiver"]["kind"], "finding");
    assert_eq!(entry["waiver"]["reason"], "checked by hand");
    // The stale waiver hides nothing: its finding is back, and it is lapsed.
    assert!(
        found.contains(&lapsing["key"].as_str().unwrap().to_string()),
        "{second}"
    );
    assert_eq!(second["lapsed"].as_array().unwrap().len(), 1, "{second}");
    assert_eq!(second["lapsed"][0]["fingerprint"], "stale");
    assert_eq!(second["lapsed"][0]["reason"], "checked by hand");
}

#[test]
fn a_finding_a_net_waiver_hides_is_listed_with_that_net_waiver() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    // A wire with a local label and no pin: a finding about the net LONELY.
    let mut kid = with_free_wire(&child(), 100.0);
    kid.labels.push(Label::new(
        "LONELY",
        Point::new(100.0, 100.0),
        LabelType::Net,
    ));
    write(&child_path(&dir), kid);
    let first = report(&mut app, &top(&dir), &[]);
    let on_net = first["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["evidence"]["net"]["name"]
                .as_str()
                .is_some_and(|n| !n.is_empty())
        })
        .unwrap_or_else(|| panic!("a finding on a named net: {first}"))
        .clone();
    let (rule, net) = (
        on_net["rule"].as_str().unwrap().to_string(),
        on_net["evidence"]["net"]["name"]
            .as_str()
            .unwrap()
            .to_string(),
    );
    let mut rules = ProjectRules::default();
    rules
        .erc_net_waivers
        .insert(rule.clone(), [net.clone()].into_iter().collect());
    set_rules(&mut app, rules);
    let second = report(&mut app, &top(&dir), &[]);
    assert!(!keys(&second, "findings").contains(&on_net["key"].as_str().unwrap().to_string()));
    let entry = second["waived"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["finding"]["key"] == on_net["key"])
        .unwrap_or_else(|| panic!("listed as waived: {second}"));
    assert_eq!(
        entry["waiver"],
        serde_json::json!({"kind": "net", "rule_id": rule, "net": net})
    );
}

/// The ERC results, the panel's flags and what Messages was told.
fn erc_state(app: &Signex) -> (u64, bool, u64, usize, usize, usize) {
    let ui = &app.ui_state;
    (
        ui.erc_run_generation,
        ui.erc_running,
        ui.erc_epoch,
        ui.erc_violations_by_path.len(),
        ui.erc_violations.len(),
        app.document_state.erc_fallback_noted.len(),
    )
}

/// Lines logged after `mark` that contain `text`. The ring is shared by every
/// test thread, so `text` is one no other test logs.
fn logged(mark: Option<u64>, text: &str) -> Vec<String> {
    crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .filter(|message| message.contains(text))
        .collect()
}

#[test]
fn a_check_leaves_the_erc_results_and_the_panel_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let before = erc_state(&app);
    let panels = crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock);
    let checked = report(&mut app, &top(&dir), &[]);
    assert!(!checked["findings"].as_array().unwrap().is_empty());
    assert_eq!(erc_state(&app), before);
    assert_eq!(
        crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock),
        panels
    );
    // Run ERC on the same project fills them.
    app.run_erc_blocking();
    assert_ne!(erc_state(&app), before);
}

#[test]
fn the_run_of_a_check_says_nothing_in_messages() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    // A child reference out of the folder: Run ERC's run says so in Messages.
    let reference = format!("../escape-{}.snxsch", Uuid::now_v7().simple());
    let mut escaping = parent().child_sheets[0].clone();
    escaping.uuid = Uuid::now_v7();
    escaping.filename = reference.clone();
    escaping.pins.clear();
    let mut kid = child();
    kid.child_sheets.push(escaping);
    write(&child_path(&dir), kid);

    // The job a `check_erc` runs off the UI thread, run here.
    let mark = crate::diagnostics::newest_entry_id();
    let job = app.erc_check_job(&top(&dir), app.document_state.projects.first());
    let checked = job();
    assert!(!checked.findings.is_empty());
    let said = logged(mark, &reference);
    assert!(said.is_empty(), "{said:?}");

    let mark = crate::diagnostics::newest_entry_id();
    app.run_erc_blocking();
    assert!(!logged(mark, &reference).is_empty(), "Run ERC says it");
}

#[test]
fn a_check_finds_what_run_erc_finds() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write(&child_path(&dir), child_with_free_wire(100.0));
    let checked = report(&mut app, &top(&dir), &[]);
    app.run_erc_blocking();
    let mut ran: Vec<(String, String)> = app
        .ui_state
        .erc_violations_by_path
        .iter()
        .flat_map(|(path, list)| {
            list.iter()
                .map(move |v| (path.display().to_string(), v.message.clone()))
        })
        .collect();
    let mut found: Vec<(String, String)> = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let text = |field: &str| f[field].as_str().unwrap().to_string();
            (text("sheet"), text("message"))
        })
        .collect();
    ran.sort();
    found.sort();
    assert!(!found.is_empty());
    assert_eq!(found, ran);
}
