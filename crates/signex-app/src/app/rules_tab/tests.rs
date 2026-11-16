//! #2398 - the Rules tab and the `rules_*` commands, driven through
//! `Signex::update` and the command dispatch: one tab per project, a cell
//! edit rewrites only its rule as one undo step, the agent commands answer
//! with JSON, and "Why this value?", the flags and the per-rule results
//! come from the board.

use std::path::PathBuf;

use serde_json::Value;
use signex_drc_dsl::table::{LimitField, RuleField, RuleKind};
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::project::{DrcRuleSet, ProjectRules};
use uuid::Uuid;

use super::{RulesDomain, RulesTabMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex, TabKind};
use crate::keymap::AppCommandId;

/// Two rules, odd spacing on purpose: an edit of one leaves the other as
/// typed.
const TEXT: &str = "rule sig_a  on clearance when net == \"SIG\"\n  require clearance >= 0.3mm then error\nrule sig_b on clearance when net == \"SIG\" require clearance >= 0.4mm then warning\n";

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

/// GND and SIG 0.35 mm apart centre to centre (0.1 mm gap).
fn board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "GND"), (2, "SIG")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![
        track(1, 1, (0.0, 0.0), (10.0, 0.0)),
        track(2, 2, (0.0, 0.35), (10.0, 0.35)),
    ];
    board
}

fn stored(text: &str) -> ProjectRules {
    ProjectRules {
        drc: DrcRuleSet {
            dsl: Some(text.into()),
            ..DrcRuleSet::default()
        },
        ..ProjectRules::default()
    }
}

/// A project in a temporary folder storing [`TEXT`], its board open.
fn app_with_board(dir: &tempfile::TempDir) -> (Signex, ProjectId, PathBuf) {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    let path = dir.path().join("board.snxpcb");
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    app.document_state.projects[0].data.rules = Some(stored(TEXT));
    app.open_pcb_tab(path.clone(), "board".into(), board());
    let id = app.document_state.projects[0].id;
    (app, id, path)
}

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn dsl(app: &Signex) -> Option<String> {
    app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|r| r.drc.dsl.clone())
}

fn send(app: &mut Signex, msg: RulesTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Rules(msg)));
}

fn command(app: &mut Signex, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog id");
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
}

fn open(app: &mut Signex) {
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        "open_rules",
    )));
}

/// Run a `rules_*` command with `operands` after `document=<project>`.
fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", project_path(app).display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    match app.run_query(&command, &args) {
        Some((outcome, _)) => outcome,
        None => app.run_operands(&command, &args).0,
    }
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn undo_steps(app: &Signex, id: ProjectId) -> usize {
    app.document_state
        .rules_history
        .get(&id)
        .map_or(0, |h| h.undo_len())
}

#[test]
fn design_rules_opens_one_rules_tab_per_project() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(tab.kind, TabKind::Rules(held) if held == id));
    assert_eq!(tab.title, "Rules");
    let tabs = app.document_state.tabs.len();
    command(&mut app, "previous_document_tab");
    open(&mut app);
    assert_eq!(
        app.document_state.tabs.len(),
        tabs,
        "focused, not opened twice"
    );
    assert_eq!(app.rules_tab_in_window(true), Some(id));
    let _ = app.view_rules_tab(id);
}

#[test]
fn a_cell_edit_rewrites_only_its_rule_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    let project = project_path(&app);
    let field = RuleField::Limit(LimitField::MinClearance);
    send(
        &mut app,
        RulesTabMsg::CellOpened {
            project_id: id,
            rule: "sig_b".into(),
            field,
        },
    );
    let _ = app.view_rules_tab(id);
    send(
        &mut app,
        RulesTabMsg::CellEdited {
            project_id: id,
            text: "0.5mm".into(),
        },
    );
    send(&mut app, RulesTabMsg::CellCommitted { project_id: id });
    let text = dsl(&app).unwrap();
    assert!(
        text.starts_with(
            "rule sig_a  on clearance when net == \"SIG\"\n  require clearance >= 0.3mm then error\n"
        ),
        "the other rule is as typed: {text}"
    );
    assert!(text.contains(
        "rule sig_b on clearance when net == \"SIG\" require clearance >= 0.5mm then warning"
    ));
    assert_eq!(undo_steps(&app, id), 1);
    assert!(app.document_state.is_document_dirty(&project));
    assert!(app.document_state.project_tabs[&id].rules.edit.is_none());

    command(&mut app, "undo");
    assert_eq!(dsl(&app).as_deref(), Some(TEXT));
    assert!(!app.document_state.is_document_dirty(&project));
    command(&mut app, "redo");
    assert!(dsl(&app).unwrap().contains("0.5mm"));
}

#[test]
fn a_refused_cell_edit_says_why_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    for (field, text) in [
        (RuleField::Limit(LimitField::MinClearance), "0.5xx"),
        (RuleField::Name, "sig_b"),
        (RuleField::Scope, "layer == outer"),
    ] {
        send(
            &mut app,
            RulesTabMsg::CellOpened {
                project_id: id,
                rule: "sig_a".into(),
                field,
            },
        );
        send(
            &mut app,
            RulesTabMsg::CellEdited {
                project_id: id,
                text: text.into(),
            },
        );
        send(&mut app, RulesTabMsg::CellCommitted { project_id: id });
        let tab = &app.document_state.project_tabs[&id].rules;
        assert!(tab.refused.is_some(), "{text} is refused");
        assert!(tab.edit.is_some(), "the cell stays open");
        let _ = app.view_rules_tab(id);
    }
    assert_eq!(dsl(&app).as_deref(), Some(TEXT));
    assert_eq!(undo_steps(&app, id), 0);
}

#[test]
fn a_project_without_rules_stores_them_with_its_first_rule() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    app.document_state.projects[0].data.rules = None;
    app.ui_state.machine_drc_rules.dsl =
        Some("rule base on clearance require clearance >= 0.2mm then error\n".into());
    open(&mut app);
    let _ = app.view_rules_tab(id);
    send(
        &mut app,
        RulesTabMsg::DomainPicked {
            project_id: id,
            domain: RulesDomain::Routing,
        },
    );
    send(
        &mut app,
        RulesTabMsg::RuleAdded {
            project_id: id,
            kind: RuleKind::Track,
        },
    );
    let text = dsl(&app).expect("the rules are stored in the project");
    assert!(text.starts_with("rule base on clearance"), "{text}");
    assert!(text.contains("rule track_1 on track require width >= 0.15mm then error"));
    assert_eq!(
        app.document_state.project_tabs[&id]
            .rules
            .selected
            .as_deref(),
        Some("track_1")
    );
    command(&mut app, "undo");
    assert!(
        app.document_state.projects[0].data.rules.is_none(),
        "one step: the project stores no rules again"
    );
}

#[test]
fn on_off_remove_and_the_text_view_are_undo_steps_too() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::EnabledToggled {
            project_id: id,
            rule: "sig_a".into(),
            on: false,
        },
    );
    assert!(dsl(&app).unwrap().starts_with("disabled rule sig_a"));
    send(
        &mut app,
        RulesTabMsg::RuleRemoved {
            project_id: id,
            rule: "sig_b".into(),
        },
    );
    assert!(!dsl(&app).unwrap().contains("sig_b"));
    assert_eq!(undo_steps(&app, id), 2);

    send(
        &mut app,
        RulesTabMsg::TextViewToggled {
            project_id: id,
            on: true,
        },
    );
    let _ = app.view_rules_tab(id);
    let typed =
        iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert('#'));
    send(
        &mut app,
        RulesTabMsg::TextActed {
            project_id: id,
            action: typed,
        },
    );
    send(&mut app, RulesTabMsg::TextApplied { project_id: id });
    assert!(dsl(&app).unwrap().starts_with('#'), "the text as typed");
    assert_eq!(undo_steps(&app, id), 3);
    let _ = app.view_rules_tab(id);
    command(&mut app, "undo");
    assert!(dsl(&app).unwrap().starts_with("disabled rule sig_a"));
}

#[test]
fn the_rules_commands_list_add_set_enable_and_remove_one_step_each() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let list = answer(&mut app, "rules_list", &[]);
    let names: Vec<&str> = list["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["sig_a", "sig_b"]);
    assert_eq!(list["stored_in"], "project");
    assert_eq!(list["rules"][0]["limits"]["min_clearance"], "0.3mm");

    let added = answer(
        &mut app,
        "rules_add",
        &[
            "kind=via",
            "values=min_drill=0.2mm",
            "scope=net == \"GND\"",
            "name=gnd_vias",
        ],
    );
    assert_eq!(added["id"], "user::gnd_vias");
    let set = answer(
        &mut app,
        "rules_set",
        &["id=gnd_vias", "field=severity", "value=info"],
    );
    assert_eq!(set["rule"]["severity"], "info");
    let off = answer(&mut app, "rules_enable", &["id=user::sig_b", "on=false"]);
    assert_eq!(off["enabled"], false);
    let removed = answer(&mut app, "rules_remove", &["id=sig_a"]);
    assert_eq!(removed["removed"], "sig_a");
    assert_eq!(undo_steps(&app, id), 4, "one undo step per edit");

    let text = dsl(&app).unwrap();
    assert!(text.contains("disabled rule sig_b"));
    assert!(
        text.contains("rule gnd_vias on via when net == \"GND\" require drill >= 0.2mm then info")
    );
    let got = answer(&mut app, "rules_get", &["id=gnd_vias"]);
    assert_eq!(got["kind"], "via");
    assert_eq!(got["scope_objects"], 0, "GND has no via");

    let refused = run(
        &mut app,
        "rules_set",
        &["id=gnd_vias", "field=min_drill", "value=2"],
    );
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
    assert_eq!(undo_steps(&app, id), 4, "a refusal records no step");
}

#[test]
fn effective_conflicts_and_explain_answer_from_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, path) = app_with_board(&dir);
    let sig = answer(&mut app, "rules_effective", &["net=SIG"]);
    assert_eq!(
        sig["source"]["rule"], "sig_b",
        "equally specific: the stricter wins"
    );
    assert_eq!(sig["value_mm"], 0.4);
    let verdicts: Vec<&str> = sig["chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["verdict"].as_str().unwrap())
        .collect();
    assert_eq!(verdicts, ["less_strict", "won"]);
    let gnd = answer(&mut app, "rules_effective", &["net=GND"]);
    assert_eq!(gnd["source"]["kind"], "board_setup");

    let flags = answer(&mut app, "rules_conflicts", &[]);
    let flags = flags["flags"].as_array().unwrap();
    assert!(
        flags
            .iter()
            .any(|f| f["rule"] == "sig_a" && f["flag"] == "shadowed")
    );
    assert!(
        flags
            .iter()
            .any(|f| f["rule"] == "sig_b" && f["flag"] == "conflict")
    );

    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let report = &app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .unwrap()
        .report;
    let index = report
        .violations
        .iter()
        .position(|v| v.rule.as_deref() == Some("user::sig_b"))
        .expect("SIG's 0.1 mm gap breaks sig_b");
    let operand = format!("violation={index}");
    let why = answer(&mut app, "rules_explain", &[operand.as_str()]);
    assert_eq!(why["rule"], "user::sig_b");
    assert_eq!(why["sides"].as_array().unwrap().len(), 2);
}

#[test]
fn the_tab_shows_flags_and_results_from_the_last_run() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    open(&mut app);
    let analysis = app.document_state.project_tabs[&id]
        .rules
        .analysis
        .clone()
        .expect("analysed on open");
    assert!(analysis.has_board);
    assert!(
        analysis
            .flags
            .iter()
            .any(|f| f.rule == "sig_a" && f.flag.as_str() == "shadowed")
    );
    assert_eq!(analysis.scope.get("sig_b"), Some(&1));
    send(
        &mut app,
        RulesTabMsg::RowSelected {
            project_id: id,
            rule: "sig_b".into(),
        },
    );
    let _ = app.view_rules_tab(id);
    send(
        &mut app,
        RulesTabMsg::ScopeShown {
            project_id: id,
            rule: "sig_b".into(),
        },
    );
    let active = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(active.kind, TabKind::Pcb), "the board is shown");
}

#[test]
fn a_rule_tab_survives_a_project_whose_board_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.rules = Some(stored(TEXT));
    let id = app.document_state.projects[0].id;
    send(&mut app, RulesTabMsg::OpenRequested { project: Some(id) });
    assert_eq!(app.rules_tab_in_window(true), Some(id));
    let analysis = app.document_state.project_tabs[&id].rules.analysis.clone();
    assert!(analysis.is_some_and(|a| !a.has_board));
    let refused = run(&mut app, "rules_effective", &["net=SIG"]);
    assert!(matches!(refused, CommandOutcome::Disabled { .. }));
    let _ = app.view_rules_tab(id);
}
