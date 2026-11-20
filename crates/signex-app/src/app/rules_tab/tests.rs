//! #2398 - the Rules tab and the `rules_*` commands, driven through
//! `Signex::update` and the command dispatch: one tab per project, a cell
//! edit rewrites only its rule as one undo step, the agent commands answer
//! with JSON, and "Why this value?", the flags and the per-rule results
//! come from the board.

use std::path::PathBuf;

use serde_json::Value;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::{LimitField, RuleField, RuleKind};
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::project::{DrcRuleSet, ProjectRules};
use uuid::Uuid;

use signex_drc_dsl::table::grid::CellSource;
use signex_widgets::constraint_grid::{CellRef, GridMessage};

use super::RulesTabMsg;
use super::grid::{CellShow, Cells, RowRef};
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
        attributes: Default::default(),
        locked: false,
    }
}

/// GND and SIG 0.35 mm apart centre to centre (0.1 mm gap), on a 2-layer
/// board: `Custom` takes the layer count (Standard is 4 layers) and the
/// standard values.
fn board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Custom
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
    assert_eq!(tab.title, "ProjectA — Rules");
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
            domain: Domain::Physical,
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
    select_sig_b(&mut app, id);
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

fn rules_tab(app: &Signex, id: ProjectId) -> &super::RulesTabState {
    &app.document_state.project_tabs[&id].rules
}

fn type_hash(app: &mut Signex, id: ProjectId) {
    let typed =
        iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert('#'));
    send(
        app,
        RulesTabMsg::TextActed {
            project_id: id,
            action: typed,
        },
    );
}

fn text_view(app: &Signex, id: ProjectId) -> String {
    rules_tab(app, id)
        .text
        .as_ref()
        .expect("the text view is open")
        .content
        .text()
}

#[test]
fn undo_and_the_table_wait_while_the_text_view_holds_typed_text() {
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
    send(
        &mut app,
        RulesTabMsg::TextViewToggled {
            project_id: id,
            on: true,
        },
    );
    type_hash(&mut app, id);

    command(&mut app, "undo");
    assert!(dsl(&app).unwrap().starts_with("disabled rule sig_a"));
    assert!(text_view(&app, id).starts_with('#'), "the typed text stays");
    assert!(
        rules_tab(&app, id).refused.is_some(),
        "and the tab says why"
    );
    send(
        &mut app,
        RulesTabMsg::TextViewToggled {
            project_id: id,
            on: false,
        },
    );
    assert!(text_view(&app, id).starts_with('#'), "the table waits too");
    let agent = run(&mut app, "undo", &[]);
    assert!(
        matches!(agent, CommandOutcome::Disabled { .. }),
        "{agent:?}"
    );
    assert_eq!(undo_steps(&app, id), 1);
    let _ = app.view_rules_tab(id);

    send(&mut app, RulesTabMsg::TextDiscarded { project_id: id });
    let tab = rules_tab(&app, id);
    assert!(!tab.text.as_ref().unwrap().edited);
    assert!(tab.refused.is_none());
    assert!(text_view(&app, id).starts_with("disabled rule sig_a"));

    command(&mut app, "undo");
    assert_eq!(dsl(&app).as_deref(), Some(TEXT));
    assert!(
        text_view(&app, id).starts_with("rule sig_a"),
        "the text view stays open on the undone text"
    );
}

#[test]
fn an_agent_undoes_a_rules_edit_by_the_project_document() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let description = app.document_state.projects[0].data.description.clone();
    answer(
        &mut app,
        "rules_set",
        &["id=sig_b", "field=severity", "value=info"],
    );
    // A Project tab edit after it is the project's newest step.
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        crate::app::ProjectTabMsg::DescriptionEdited {
            project_id: id,
            text: "edited".into(),
        },
    )));
    assert_ne!(app.document_state.projects[0].data.description, description);

    let first = run(&mut app, "undo", &[]);
    assert!(matches!(first, CommandOutcome::Ran), "{first:?}");
    assert_eq!(app.document_state.projects[0].data.description, description);
    assert!(dsl(&app).unwrap().contains("then info"), "the rules wait");

    let second = run(&mut app, "undo", &[]);
    assert!(matches!(second, CommandOutcome::Ran), "{second:?}");
    assert_eq!(dsl(&app).as_deref(), Some(TEXT));
    let none = run(&mut app, "undo", &[]);
    assert!(matches!(none, CommandOutcome::Disabled { .. }), "{none:?}");

    let redone = run(&mut app, "redo", &[]);
    assert!(matches!(redone, CommandOutcome::Ran), "{redone:?}");
}

#[test]
fn severity_entries_follow_the_tab_and_the_commands() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let entries = |app: &Signex| {
        app.document_state.projects[0]
            .data
            .rules
            .as_ref()
            .unwrap()
            .drc
            .severity
            .clone()
    };
    {
        let drc = &mut app.document_state.projects[0]
            .data
            .rules
            .as_mut()
            .unwrap()
            .drc;
        drc.severity.insert("user::sig_a".into(), "off".into());
        drc.severity.insert("user::sig_b".into(), "info".into());
    }
    let list = answer(&mut app, "rules_list", &[]);
    assert_eq!(list["rules"][0]["effective_severity"], "off");
    assert_eq!(list["rules"][1]["severity"], "warning");
    assert_eq!(list["rules"][1]["effective_severity"], "info");

    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::CellOpened {
            project_id: id,
            rule: "sig_b".into(),
            field: RuleField::Severity,
        },
    );
    let opened = rules_tab(&app, id).edit.clone().unwrap();
    assert_eq!(opened.text, "info", "the cell opens on what applies");
    send(&mut app, RulesTabMsg::CellCancelled { project_id: id });
    let _ = app.view_rules_tab(id);

    // On in the text, off by its entry: switching it on clears the entry.
    send(
        &mut app,
        RulesTabMsg::EnabledToggled {
            project_id: id,
            rule: "sig_a".into(),
            on: true,
        },
    );
    assert!(!entries(&app).contains_key("user::sig_a"));
    assert_eq!(undo_steps(&app, id), 1);

    answer(
        &mut app,
        "rules_set",
        &["id=sig_b", "field=name", "value=sig_c"],
    );
    let moved = entries(&app);
    assert_eq!(moved.get("user::sig_c").map(String::as_str), Some("info"));
    assert!(!moved.contains_key("user::sig_b"), "{moved:?}");

    let set = answer(
        &mut app,
        "rules_set",
        &["id=sig_c", "field=severity", "value=error"],
    );
    assert_eq!(set["rule"]["effective_severity"], "error");
    assert!(entries(&app).is_empty(), "its own word governs again");
    command(&mut app, "undo");
    assert_eq!(
        entries(&app).get("user::sig_c").map(String::as_str),
        Some("info"),
        "the entry comes back in the same step"
    );

    send(
        &mut app,
        RulesTabMsg::RuleRemoved {
            project_id: id,
            rule: "sig_c".into(),
        },
    );
    assert!(entries(&app).is_empty(), "a removed rule leaves no entry");
}

#[test]
fn update_keeps_the_selected_rules_why_and_the_text_view_follows_stored_rules() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    open(&mut app);
    select_sig_b(&mut app, id);
    let analysis = rules_tab(&app, id).analysis.clone().expect("analysed");
    let (effective, net) = analysis.why.expect("why for the selected rule");
    assert_eq!(net, "SIG");
    assert_eq!(effective.value_nm, Some(400_000));
    assert!(matches!(
        analysis.checked,
        super::analysis::Checked::Ran { stale: false, .. }
    ));
    let _ = app.view_rules_tab(id);

    send(
        &mut app,
        RulesTabMsg::TextViewToggled {
            project_id: id,
            on: true,
        },
    );
    answer(
        &mut app,
        "rules_set",
        &["id=sig_b", "field=min_clearance", "value=0.5mm"],
    );
    // Any message runs `update`, which refreshes the text view.
    send(&mut app, RulesTabMsg::InheritedToggled { project_id: id });
    assert!(
        text_view(&app, id).contains("clearance >= 0.5mm"),
        "a text view nobody typed in shows the stored rules"
    );
}

// #2530 - the constraint grid: domains, find, cell edits, reset, the
// filter chips and constraint sets on rows.

fn grid(app: &Signex, id: ProjectId) -> &super::RulesGrid {
    &rules_tab(app, id).grid
}

fn grid_msg(app: &mut Signex, id: ProjectId, msg: GridMessage) {
    send(
        app,
        RulesTabMsg::GridActed {
            project_id: id,
            msg,
        },
    );
}

fn pick(app: &mut Signex, id: ProjectId, domain: Domain) {
    send(
        app,
        RulesTabMsg::DomainPicked {
            project_id: id,
            domain,
        },
    );
}

/// The cell of row `row` in the column with catalog id `column`.
fn cell(app: &Signex, id: ProjectId, row: &RowRef, column: &str) -> CellRef {
    let grid = grid(app, id);
    CellRef::new(
        grid.row_of(row).expect("the row is listed"),
        grid.column_named(column)
            .expect("the column is on the domain"),
    )
}

/// Select the SIG row's clearance cell, which `sig_b` (the stricter of
/// the two SIG rules) governs, and close its editor.
fn select_sig_b(app: &mut Signex, id: ProjectId) {
    let at = cell(app, id, &RowRef::Net("SIG".into()), "clearance_min");
    grid_msg(app, id, GridMessage::CellPressed(at));
    grid_msg(app, id, GridMessage::EditCancelled);
    assert_eq!(rules_tab(app, id).selected.as_deref(), Some("sig_b"));
}

/// Type `value` in `at` and press Enter.
fn type_in(app: &mut Signex, id: ProjectId, at: CellRef, value: &str) {
    grid_msg(app, id, GridMessage::CellPressed(at));
    grid_msg(app, id, GridMessage::EditTextChanged(value.into()));
    grid_msg(app, id, GridMessage::EditSubmitted);
}

#[test]
fn a_domain_pick_lays_out_its_catalog_columns_and_hierarchy_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    assert_eq!(grid(&app, id).domain(), Domain::Clearance, "the first tab");
    pick(&mut app, id, Domain::Physical);
    let shown = grid(&app, id);
    assert_eq!(shown.domain(), Domain::Physical);
    let groups: Vec<&str> = shown.groups().iter().map(|g| g.id).collect();
    let catalog: Vec<&str> = Domain::Physical.groups().map(|g| g.id).collect();
    assert_eq!(groups, catalog, "the groups are the catalog's, in order");
    let rows = shown.rows();
    assert_eq!(rows[0], RowRef::All);
    assert!(
        rows.contains(&RowRef::Net("SIG".into())),
        "a net with rules of its own is listed: {rows:?}"
    );
    let layers: Vec<(usize, usize)> = rows
        .iter()
        .filter_map(|r| match r {
            RowRef::Layer { number, count, .. } => Some((*number, *count)),
            _ => None,
        })
        .collect();
    assert_eq!(layers, vec![(1, 2), (2, 2)], "L1 is Top, L2 is Bottom");
    // Placement has no net or layer rows.
    pick(&mut app, id, Domain::Placement);
    assert!(
        grid(&app, id)
            .rows()
            .iter()
            .all(|r| !matches!(r, RowRef::Net(_) | RowRef::Layer { .. })),
        "{:?}",
        grid(&app, id).rows()
    );
}

#[test]
fn find_jumps_to_the_matching_column_and_enter_steps_to_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    grid_msg(
        &mut app,
        id,
        GridMessage::ColumnQueryChanged("annular".into()),
    );
    let shown = grid(&app, id);
    assert_eq!(shown.domain(), Domain::Manufacturing, "the column's domain");
    let column = shown
        .column_named("annular_ring_min")
        .expect("on the domain");
    assert_eq!(shown.view.selected().map(|c| c.column), Some(column));
    assert_eq!(
        shown.view.selected().map(|c| c.row),
        shown.row_of(&RowRef::All),
        "no row was selected, so All"
    );
    // Two letters only list; the grid stays.
    grid_msg(&mut app, id, GridMessage::ColumnQueryChanged("wi".into()));
    assert_eq!(grid(&app, id).domain(), Domain::Manufacturing);
    assert!(rules_tab(&app, id).find.is_listing());
    grid_msg(
        &mut app,
        id,
        GridMessage::ColumnQueryChanged("width".into()),
    );
    let shown = grid(&app, id);
    assert_eq!(shown.domain(), Domain::Physical);
    assert_eq!(
        shown.view.selected().map(|c| c.column),
        shown.column_named("width_min")
    );
    grid_msg(&mut app, id, GridMessage::ColumnQuerySubmitted);
    let shown = grid(&app, id);
    assert_eq!(
        shown.view.selected().map(|c| c.column),
        shown.column_named("width_preferred"),
        "Enter steps to the next match"
    );
}

#[test]
fn a_cell_commit_writes_one_rule_and_one_undo_step_and_reset_removes_only_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    let before = undo_steps(&app, id);
    let at = cell(&app, id, &RowRef::All, "clearance_min");
    type_in(&mut app, id, at, "0.25");
    let text = dsl(&app).expect("stored");
    assert!(
        text.starts_with(TEXT),
        "the written rules stay as typed: {text}"
    );
    assert!(
        text.contains("rule all_clearance on clearance require clearance >= 0.25mm then error"),
        "{text}"
    );
    assert_eq!(undo_steps(&app, id), before + 1, "one undo step");
    assert!(grid(&app, id).view.edit().is_none(), "the editor closed");
    // A bare number is millimetres; the cell shows the row's own value.
    let tab = rules_tab(&app, id);
    let sets = std::collections::BTreeMap::new();
    let table = signex_drc_dsl::table::table(&text);
    let cells = Cells {
        grid: &tab.grid,
        table: &table,
        sets: &sets,
    };
    assert!(cells.show(at).is_own());

    grid_msg(&mut app, id, GridMessage::CellReset(at));
    assert_eq!(
        dsl(&app).as_deref(),
        Some(TEXT),
        "reset drops that rule only"
    );
    assert_eq!(undo_steps(&app, id), before + 2);
    command(&mut app, "undo");
    assert!(dsl(&app).expect("stored").contains("all_clearance"));
    command(&mut app, "undo");
    assert_eq!(dsl(&app).as_deref(), Some(TEXT));
}

#[test]
fn overridden_rows_only_lists_the_rows_with_values_of_their_own() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    let has_layers = |app: &Signex| {
        grid(app, id)
            .rows()
            .iter()
            .any(|r| matches!(r, RowRef::Layer { .. }))
    };
    assert!(has_layers(&app));
    send(
        &mut app,
        RulesTabMsg::OverriddenOnlyToggled { project_id: id },
    );
    let rows = grid(&app, id).rows().to_vec();
    assert!(rows.contains(&RowRef::All), "All always stays");
    assert!(rows.contains(&RowRef::Net("SIG".into())), "{rows:?}");
    assert!(!has_layers(&app), "no layer sets a clearance: {rows:?}");
    send(
        &mut app,
        RulesTabMsg::OverriddenOnlyToggled { project_id: id },
    );
    assert!(has_layers(&app));
    // Show inherited values starts on.
    assert!(!rules_tab(&app, id).hide_inherited);
    send(&mut app, RulesTabMsg::InheritedToggled { project_id: id });
    assert!(rules_tab(&app, id).hide_inherited);
}

#[test]
fn a_row_using_a_constraint_set_carries_its_chip_and_resolves_from_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    answer(
        &mut app,
        "rules_set_create",
        &["name=hv", "values=min_clearance=0.8mm"],
    );
    answer(
        &mut app,
        "rules_set_apply",
        &["set=hv", "scope=net == \"GND\""],
    );
    // The next `update` lays the rows out again.
    pick(&mut app, id, Domain::Clearance);
    let gnd = RowRef::Net("GND".into());
    let drc = app.rules_drc_of(id).expect("rules");
    assert_eq!(
        super::grid::sets_on(&gnd, &drc.sets),
        vec!["hv".to_string()]
    );
    assert!(super::grid::sets_on(&RowRef::Net("SIG".into()), &drc.sets).is_empty());
    let at = cell(&app, id, &gnd, "clearance_min");
    let table = signex_drc_dsl::table::table(drc.dsl.as_deref().unwrap_or_default());
    let cells = Cells {
        grid: grid(&app, id),
        table: &table,
        sets: &drc.sets,
    };
    match cells.show(at) {
        CellShow::Value { source, .. } => assert!(
            matches!(&source, CellSource::Set { set, .. } if set == "hv"),
            "{source:?}"
        ),
        other => panic!("the set's value: {other:?}"),
    }
}

/// The project's stored rules replaced with `text`, before the tab opens.
fn store(app: &mut Signex, text: &str) {
    app.document_state.projects[0].data.rules = Some(stored(text));
}

fn layer_rows(app: &Signex, id: ProjectId) -> Vec<RowRef> {
    grid(app, id)
        .rows()
        .iter()
        .filter(|r| matches!(r, RowRef::Layer { .. }))
        .cloned()
        .collect()
}

#[test]
fn a_layer_rule_for_a_layer_the_board_lacks_keeps_its_row() {
    let outer = "rule outer_w on track when layer == outer require width >= 0.25mm then error\n";
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    store(&mut app, outer);
    open(&mut app);
    pick(&mut app, id, Domain::Physical);
    let rows = layer_rows(&app, id);
    assert_eq!(
        rows.len(),
        3,
        "the two board layers and the written one: {rows:?}"
    );
    assert_eq!(
        rows[2],
        RowRef::Layer {
            token: "outer".into(),
            number: 0,
            count: 2,
        }
    );
    let _ = app.view_rules_tab(id);

    // With no board open, the written layer is still listed.
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    store(&mut app, outer);
    let id = app.document_state.projects[0].id;
    send(&mut app, RulesTabMsg::OpenRequested { project: Some(id) });
    pick(&mut app, id, Domain::Physical);
    assert_eq!(
        layer_rows(&app, id),
        [RowRef::Layer {
            token: "outer".into(),
            number: 0,
            count: 0,
        }]
    );
    let _ = app.view_rules_tab(id);
}

#[test]
fn a_board_wide_rule_no_cell_shows_is_a_scoped_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    store(
        &mut app,
        "rule wide_a on clearance require clearance >= 0.2mm then error\n\
         rule wide_b on clearance require clearance >= 0.3mm then error\n",
    );
    open(&mut app);
    let rows = grid(&app, id).rows();
    assert!(rows.contains(&RowRef::Scoped("wide_a".into())), "{rows:?}");
    assert!(!rows.contains(&RowRef::Scoped("wide_b".into())), "{rows:?}");
    let _ = app.view_rules_tab(id);
}

#[test]
fn a_cell_shows_its_length_as_written_in_any_unit_the_rules_take() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    open(&mut app);
    let at = cell(&app, id, &RowRef::All, "clearance_min");
    for (typed, mm) in [("8mil", 0.2032), ("50\u{b5}m", 0.05), ("50\u{3bc}m", 0.05)] {
        type_in(&mut app, id, at, typed);
        let text = dsl(&app).expect("stored");
        assert!(
            text.contains(&format!("clearance >= {typed} then")),
            "{text}"
        );
        let tab = rules_tab(&app, id);
        let sets = std::collections::BTreeMap::new();
        let table = signex_drc_dsl::table::table(&text);
        let cells = Cells {
            grid: &tab.grid,
            table: &table,
            sets: &sets,
        };
        let show = cells.show(at);
        assert_eq!(
            show.written(),
            Some(typed),
            "the cell shows {typed}, not mm"
        );
        assert!(
            matches!(show, CellShow::Value { mm: Some(v), .. } if (v - mm).abs() < 1e-9),
            "{typed} reads as {mm} mm: {show:?}"
        );
        let _ = app.view_rules_tab(id);
    }
}

/// The Clearance tab's and the clearance group's findings badges, as the
/// view reads them.
fn clearance_badges(app: &Signex, id: ProjectId) -> (Option<usize>, Option<usize>) {
    let state = rules_tab(app, id);
    let (src, origin) = app.rules_text_of(id).expect("rules");
    let frame = app.rules_frame(
        id,
        &app.ui_state.i18n,
        &app.document_state.panel_ctx.tokens,
        state,
        src,
        origin,
    );
    let group = signex_drc_dsl::table::catalog::group_of(LimitField::MinClearance)
        .expect("the clearance group");
    (
        frame.domain_findings(Domain::Clearance),
        frame.group_findings(group),
    )
}

#[test]
fn a_findings_badge_drops_a_rule_changed_since_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id, _) = app_with_board(&dir);
    let task = app.handle_pcb_drc_requested();
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    open(&mut app);
    let sig_b = match &rules_tab(&app, id)
        .analysis
        .as_ref()
        .expect("analysed")
        .checked
    {
        super::analysis::Checked::Ran { results, .. } => {
            results.get("user::sig_b").map_or(0, |r| r.violations)
        }
        other => panic!("a run: {other:?}"),
    };
    assert!(sig_b > 0, "sig_b asks 0.4 mm of a 0.1 mm gap");
    let (domain, group) = clearance_badges(&app, id);
    let domain = domain.expect("a current run");
    assert_eq!(group, Some(domain), "every clearance rule is in the group");

    answer(
        &mut app,
        "rules_set",
        &["id=sig_b", "field=min_clearance", "value=0.5mm"],
    );
    // Any message runs `update`, which analyses the rules again.
    send(&mut app, RulesTabMsg::InheritedToggled { project_id: id });
    let (domain_after, group_after) = clearance_badges(&app, id);
    assert_eq!(
        domain_after,
        Some(domain - sig_b),
        "sig_b changed since the run"
    );
    assert_eq!(group_after, Some(domain - sig_b));
}
