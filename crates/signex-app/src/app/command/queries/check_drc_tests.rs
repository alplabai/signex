//! #2199 (decisions 6817821301, 6818011610): `check_drc` runs DRC on one
//! board off the UI thread and answers with a check report; it leaves the
//! DRC panel, the DRC state and Messages alone, and finds what Run DRC finds.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_types::format::SnxPcb;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::project::{DrcRuleSet, ProjectRules};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::project_app;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::LOOSE_BOARD_WARNING;
use crate::keymap::AppCommandId;

const BOARD: &str = "board.snxpcb";

/// Track `n` (a fixed uuid, so a rewritten board keeps its items) of `net`.
fn track(n: u128, net: u32, a: (f64, f64), b: (f64, f64)) -> Segment {
    Segment {
        uuid: Uuid::from_u128(0x2000 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Nets GND, VCC, SIG and AUX on the Standard setup (0.15 mm clearance):
/// GND and VCC 0.1 mm apart (a clearance), AUX over GND (a short), SIG
/// 0.6 mm from VCC (clean).
fn sample_board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "GND"), (2, "VCC"), (3, "SIG"), (4, "AUX")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![
        track(1, 1, (0.0, 0.0), (10.0, 0.0)),
        track(2, 2, (0.0, 0.35), (10.0, 0.35)),
        track(3, 3, (0.0, 1.2), (10.0, 1.2)),
        track(4, 4, (5.0, -0.1), (5.0, -5.0)),
    ];
    board
}

fn write_board(path: &Path, board: PcbBoard) {
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(path, text).expect("write board");
}

/// [`project_app`] with `board.snxpcb` listed and written, not opened.
fn board_app(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut app = project_app(dir);
    let path = dir.path().join(BOARD);
    write_board(&path, sample_board());
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    (app, path)
}

fn start(
    app: &mut Signex,
    document: &Path,
    extra: &[&str],
) -> (CommandOutcome, iced::Task<crate::app::Message>) {
    let id = AppCommandId::new("check_drc").expect("a catalog id");
    let document = format!("document={}", document.display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
    app.run_query(&id, &args).expect("a query")
}

fn check(app: &mut Signex, document: &Path, extra: &[&str]) -> CommandOutcome {
    match start(app, document, extra) {
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
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn findings(report: &Value) -> &Vec<Value> {
    report["findings"].as_array().expect("findings")
}

/// `(rule, items)` of each finding, sorted.
fn rules_and_items(report: &Value) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = findings(report)
        .iter()
        .map(|f| {
            let items = f["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_str().unwrap().to_string())
                .collect();
            (f["rule"].as_str().unwrap().to_string(), items)
        })
        .collect();
    out.sort();
    out
}

fn warnings(report: &Value) -> Vec<String> {
    report["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect()
}

/// Run DRC on the open board at `path`, to its end: what it reported.
fn run_drc(app: &mut Signex, path: &Path) -> signex_drc::DrcReport {
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the run lands");
    let done = app.interaction_state.pcb_drc.done_for(path);
    done.expect("Run DRC reported").report.clone()
}

/// What Run DRC found, as `(rule, items)` in the check's words, sorted.
fn run_drc_rules_and_items(report: &signex_drc::DrcReport) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = report
        .violations
        .iter()
        .map(|v| {
            let rule = v.rule.clone().unwrap_or(v.kind.rule_id().to_string());
            let items = v
                .items
                .iter()
                .map(|item| match item {
                    pcb_geom::ItemRef::Pad { footprint, pad } => format!("{footprint}/{pad}"),
                    pcb_geom::ItemRef::Track(id)
                    | pcb_geom::ItemRef::Via(id)
                    | pcb_geom::ItemRef::Footprint(id) => id.to_string(),
                })
                .collect();
            (rule, items)
        })
        .collect();
    out.sort();
    out
}

fn open_board(app: &mut Signex, path: &Path) {
    let board = SnxPcb::parse(&std::fs::read_to_string(path).unwrap())
        .unwrap()
        .board;
    app.open_pcb_tab(path.to_path_buf(), BOARD.into(), board);
}

fn set_rules(app: &mut Signex, rules: ProjectRules) {
    app.document_state.projects[0].data.rules = Some(rules);
}

fn dsl(text: &str) -> DrcRuleSet {
    DrcRuleSet {
        dsl: Some(text.into()),
        ..DrcRuleSet::default()
    }
}

#[test]
fn a_check_of_a_closed_board_answers_with_the_report_read_off_the_ui_thread() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let (outcome, task) = start(&mut app, &path, &[]);
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("not deferred: {outcome:?}");
    };
    // Written after `update` returned: only a read in the run can see it.
    let mut moved = sample_board();
    moved.segments.truncate(1);
    write_board(&path, moved);
    let CommandOutcome::Answered(report) = app.settle_deferred(ticket, task).0 else {
        panic!("not answered");
    };
    assert_eq!(report["check"], "drc", "{report}");
    assert_eq!(report["run"], 1, "{report}");
    assert!(findings(&report).is_empty(), "{report}");
    assert!(report["waived"].as_array().unwrap().is_empty());
    assert!(report["lapsed"].as_array().unwrap().is_empty());
    assert_eq!(report["rule_set"]["source"], "machine", "{report}");
}

#[test]
fn a_check_finds_what_run_drc_finds_on_the_same_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let closed = report(&mut app, &path, &[]);
    open_board(&mut app, &path);
    let open = report(&mut app, &path, &[]);
    let ran = run_drc(&mut app, &path);
    let found = rules_and_items(&open);
    assert!(
        found.iter().any(|(rule, _)| rule == "builtin::clearance")
            && found.iter().any(|(rule, _)| rule == "builtin::short"),
        "{open}"
    );
    assert_eq!(found, run_drc_rules_and_items(&ran));
    // Closed or open, the same board gives the same findings and keys.
    assert_eq!(findings(&closed), findings(&open));
    let sheet = path.display().to_string();
    assert!(findings(&open).iter().all(|f| f["sheet"] == sheet.as_str()));
}

#[test]
fn an_open_board_is_checked_as_it_is_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    open_board(&mut app, &path);
    // The file loses the clash; the open board still has it.
    let mut clean = sample_board();
    clean.segments.truncate(1);
    write_board(&path, clean);
    let checked = report(&mut app, &path, &[]);
    assert!(!findings(&checked).is_empty(), "{checked}");
}

#[test]
fn a_project_rule_layer_is_applied_with_its_severity() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let before = report(&mut app, &path, &[]);
    assert!(
        findings(&before)
            .iter()
            .all(|f| !f["rule"].as_str().unwrap().starts_with("user::"))
    );
    // The machine layer is not read once the project stores rules.
    app.ui_state.machine_drc_rules =
        dsl("rule machine on clearance require clearance >= 5mm then error");
    set_rules(
        &mut app,
        ProjectRules {
            drc: dsl("rule wide on clearance require clearance >= 1mm then warning"),
            ..ProjectRules::default()
        },
    );
    let after = report(&mut app, &path, &[]);
    let written: Vec<&Value> = findings(&after)
        .iter()
        .filter(|f| f["rule"] == "user::wide")
        .collect();
    assert!(!written.is_empty(), "{after}");
    assert!(written.iter().all(|f| f["severity"] == "warning"));
    assert!(
        findings(&after)
            .iter()
            .all(|f| f["rule"] != "user::machine")
    );
    assert_eq!(after["rule_set"]["source"], "project", "{after}");
    assert_eq!(
        after["rule_set"]["sha256"],
        dsl("rule wide on clearance require clearance >= 1mm then warning").sha256_hex()
    );
}

#[test]
fn a_loose_board_is_checked_with_the_machine_layer_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let loose = dir.path().join("elsewhere").join("loose.snxpcb");
    app.open_pcb_tab(loose.clone(), "loose.snxpcb".into(), sample_board());
    app.ui_state.machine_drc_rules =
        dsl("rule loose_wide on clearance require clearance >= 1mm then info");
    let checked = report(&mut app, &loose, &[]);
    assert_eq!(warnings(&checked)[0], LOOSE_BOARD_WARNING, "{checked}");
    assert!(
        findings(&checked)
            .iter()
            .any(|f| f["rule"] == "user::loose_wide" && f["severity"] == "info"),
        "{checked}"
    );
    assert_eq!(checked["rule_set"]["source"], "machine");
}

#[test]
fn a_board_in_a_project_has_no_loose_board_warning() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let checked = report(&mut app, &path, &[]);
    assert!(!warnings(&checked).iter().any(|w| w == LOOSE_BOARD_WARNING));
}

#[test]
fn a_sheet_is_refused_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = board_app(&dir);
    let why = refusal(check(&mut app, &dir.path().join("top.snxsch"), &[]));
    assert!(why.contains("checks a board"), "{why}");
    assert_eq!(app.document_state.agent_edits.tickets, 0, "no run started");
}

#[test]
fn a_baseline_lists_the_new_and_the_gone_findings_of_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let first = report(&mut app, &path, &[]);
    let first_keys: Vec<Value> = findings(&first).iter().map(|f| f["key"].clone()).collect();
    // The short goes away, a new clash with SIG comes in.
    let mut next = sample_board();
    next.segments.pop();
    next.segments.push(track(5, 4, (0.0, 1.5), (10.0, 1.5)));
    write_board(&path, next);
    let second = report(&mut app, &path, &["baseline=1"]);
    assert_eq!(second["run"], 2);
    let base = &second["baseline"];
    assert_eq!(base["run"], 1);
    assert!(!base["new"].as_array().unwrap().is_empty(), "{second}");
    let gone = base["gone"].as_array().unwrap();
    assert_eq!(gone.len(), 1, "{second}");
    assert!(first_keys.contains(&gone[0]));
}

#[test]
fn a_baseline_of_another_board_or_an_erc_run_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let other = dir.path().join("elsewhere").join("other.snxpcb");
    app.open_pcb_tab(other.clone(), "other.snxpcb".into(), sample_board());
    let _ = report(&mut app, &other, &[]);
    let why = refusal(check(&mut app, &path, &["baseline=1"]));
    assert!(why.contains("other.snxpcb"), "{why}");
    let erc = app.document_state.check_runs.record(
        "erc",
        &app.document_state.projects[0].path.clone(),
        Vec::new(),
    );
    let why = refusal(check(&mut app, &path, &[&format!("baseline={erc}")]));
    assert!(why.contains("`erc` check"), "{why}");
}

/// The DRC state and panel, as text.
fn drc_state(app: &Signex) -> (String, Option<crate::panels::DrcPanel>) {
    (
        format!("{:?}", app.interaction_state.pcb_drc),
        app.pcb_drc_panel(),
    )
}

#[test]
fn a_check_leaves_the_drc_state_and_the_panel_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    open_board(&mut app, &path);
    let before = drc_state(&app);
    let panels = crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock);
    let checked = report(&mut app, &path, &[]);
    assert!(!findings(&checked).is_empty());
    assert_eq!(drc_state(&app), before);
    assert_eq!(
        crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock),
        panels
    );
    // Run DRC on the same board fills them.
    let _ = run_drc(&mut app, &path);
    assert_ne!(drc_state(&app), before);
}

/// Lines logged after `mark` that contain `text`.
fn logged(mark: Option<u64>, text: &str) -> Vec<String> {
    crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .filter(|message| message.contains(text))
        .collect()
}

#[test]
fn a_check_says_its_rule_text_in_the_report_not_in_messages() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    open_board(&mut app, &path);
    let name = format!("nowhere_{}", Uuid::now_v7().simple());
    let text = format!(
        "rule {name} on clearance when net == \"NONE\" require clearance >= 1mm then error\nrule bad on via"
    );
    set_rules(
        &mut app,
        ProjectRules {
            drc: dsl(&text),
            ..ProjectRules::default()
        },
    );
    let mark = crate::diagnostics::newest_entry_id();
    let checked = report(&mut app, &path, &[]);
    assert_eq!(logged(mark, "DRC"), Vec::<String>::new());
    assert!(
        warnings(&checked)
            .iter()
            .any(|w| w == "DRC rules: the rule text was not applied (syntax error)"),
        "{checked}"
    );
    assert!(
        warnings(&checked)
            .iter()
            .any(|w| w.starts_with("DRC DSL (project): ")),
        "{checked}"
    );
    // Run DRC says the same in Messages.
    let mark = crate::diagnostics::newest_entry_id();
    let _ = run_drc(&mut app, &path);
    assert!(
        !logged(mark, "the rule text was not applied").is_empty(),
        "Run DRC says it"
    );
}

#[test]
fn a_rule_scope_that_matches_no_net_is_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    let name = format!("nowhere_{}", Uuid::now_v7().simple());
    set_rules(
        &mut app,
        ProjectRules {
            drc: dsl(&format!(
                "rule {name} on clearance when net == \"NONE\" require clearance >= 1mm then error"
            )),
            ..ProjectRules::default()
        },
    );
    let mark = crate::diagnostics::newest_entry_id();
    let checked = report(&mut app, &path, &[]);
    assert_eq!(logged(mark, &name), Vec::<String>::new());
    assert!(
        warnings(&checked).iter().any(|w| w.contains(&name)),
        "{checked}"
    );
}

/// The project of [`board_app`] listing a board `file` instead.
fn listing(dir: &tempfile::TempDir, file: &str) -> Signex {
    let (mut app, _) = board_app(dir);
    app.document_state.projects[0].data.pcb_file = Some(file.into());
    app
}

/// The job of a check of `path`, built as an agent's.
fn agent_job(app: &mut Signex, path: &Path) -> Result<signex_types::check::CheckReport, String> {
    app.agent.serving = true;
    let job = app.drc_check_job(path, app.document_state.projects.first());
    app.agent.serving = false;
    job()
}

#[test]
fn an_agent_check_does_not_read_a_board_outside_the_project_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = format!("outside-{}.snxpcb", Uuid::now_v7().simple());
    let outside = dir.path().parent().unwrap().join(&name);
    write_board(&outside, sample_board());
    let mut app = listing(&dir, &format!("../{name}"));
    let path = dir.path().join(format!("../{name}"));
    let why = agent_job(&mut app, &path).unwrap_err();
    // A person in the GUI may read it.
    let job = app.drc_check_job(&path, app.document_state.projects.first());
    let person = job();
    std::fs::remove_file(&outside).ok();
    assert!(why.contains("outside the project folder"), "{why}");
    assert!(person.is_ok(), "{person:?}");
}

#[test]
fn an_agent_check_does_not_read_an_oversized_board() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = listing(&dir, "big.snxpcb");
    let path = dir.path().join("big.snxpcb");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(crate::headless::MAX_FILE_BYTES + 1).unwrap();
    let why = agent_job(&mut app, &path).unwrap_err();
    assert!(why.contains("larger than"), "{why}");
}

#[allow(
    clippy::disallowed_methods,
    reason = "the test makes a FIFO with mkfifo"
)]
#[cfg(unix)]
#[test]
fn an_agent_check_does_not_read_a_fifo() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = listing(&dir, "pipe.snxpcb");
    let path = dir.path().join("pipe.snxpcb");
    let made = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    let why = agent_job(&mut app, &path).unwrap_err();
    assert!(why.contains("not a regular file"), "{why}");
}

#[test]
fn a_board_that_cannot_be_read_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = board_app(&dir);
    std::fs::write(&path, "not a board").unwrap();
    let why = refusal(check(&mut app, &path, &[]));
    assert!(why.contains("could not be read"), "{why}");
    // A refused run is not numbered or kept.
    assert!(
        app.document_state
            .check_runs
            .keys_of(1, "drc", &path)
            .is_err()
    );
}
