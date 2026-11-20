//! #2398 steps 5 and 6 - the class matrix, named rule sets and presets,
//! driven through `Signex::update` (the tab) and the command dispatch (the
//! agent): each edit is one undo step on the project's rule history and
//! rewrites only the rules it owns; the commands answer with JSON.

use std::path::PathBuf;

use serde_json::Value;
use signex_drc_dsl::table::matrix::CellKey;
use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::project::{DrcRuleSet, ProjectRules, RuleProfile};
use uuid::Uuid;

use super::{ClearanceView, RulesPanel, RulesTabMsg, SetFormField};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::keymap::AppCommandId;

/// One rule the matrix, set and preset edits must leave as typed.
const TEXT: &str = "rule base  on clearance\n  require clearance >= 0.2mm then error\n";

fn track(n: u128, net: u32, y: f64) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(0x3000 + n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// `HV_A` (class `HV`, 0.5 mm) and `LV_A` (class `LV`), 0.4 mm apart.
fn board() -> PcbBoard {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = [(1, "HV_A"), (2, "LV_A")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    board.segments = vec![track(1, 1, 0.0), track(2, 2, 0.4)];
    board
}

fn classes() -> Vec<NetClass> {
    let mut hv = NetClass::named("HV");
    hv.nets = vec!["HV_A".into()];
    hv.clearance = 0.5;
    let mut lv = NetClass::named("LV");
    lv.nets = vec!["LV_A".into()];
    vec![hv, lv]
}

fn app_with_board(dir: &tempfile::TempDir) -> (Signex, ProjectId) {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    let path = dir.path().join("board.snxpcb");
    let project = &mut app.document_state.projects[0].data;
    project.pcb_file = Some("board.snxpcb".into());
    project.net_classes = classes();
    project.rules = Some(ProjectRules {
        drc: DrcRuleSet {
            dsl: Some(TEXT.into()),
            ..DrcRuleSet::default()
        },
        ..ProjectRules::default()
    });
    app.open_pcb_tab(path, "board".into(), board());
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn drc(app: &Signex) -> DrcRuleSet {
    app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .map(|r| r.drc.clone())
        .unwrap_or_default()
}

fn dsl(app: &Signex) -> String {
    drc(app).dsl.unwrap_or_default()
}

fn send(app: &mut Signex, msg: RulesTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Rules(msg)));
}

fn open(app: &mut Signex) {
    let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(
        "open_rules",
    )));
}

fn command(app: &mut Signex, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog id");
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
}

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

fn type_in(app: &mut Signex, id: ProjectId, field: SetFormField, text: &str) {
    send(
        app,
        RulesTabMsg::SetFormEdited {
            project_id: id,
            field,
            text: text.into(),
        },
    );
}

#[test]
fn a_matrix_cell_typed_in_writes_one_rule_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::ClearanceViewPicked {
            project_id: id,
            view: ClearanceView::Matrix,
        },
    );
    let _ = app.view_rules_tab(id);
    // An empty cell falls back, greyed, to what the pair needs without it:
    // the board-wide `base` 0.2 mm, which wins over the HV class value.
    let analysis = app.document_state.project_tabs[&id]
        .rules
        .analysis
        .as_ref()
        .expect("the tab in view is analysed");
    let fallback = analysis
        .defaults
        .get(&("HV".to_string(), "LV".to_string(), None))
        .copied();
    assert_eq!(fallback, Some(200_000));

    send(
        &mut app,
        RulesTabMsg::MatrixCellOpened {
            project_id: id,
            key: CellKey::new("LV", "HV"),
        },
    );
    let _ = app.view_rules_tab(id);
    send(
        &mut app,
        RulesTabMsg::MatrixCellEdited {
            project_id: id,
            text: "0.8mm".into(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellCommitted { project_id: id },
    );
    let text = dsl(&app);
    assert!(text.starts_with(TEXT), "the other rule is as typed: {text}");
    assert!(text.contains(
        "rule cc_LV_HV on clearance when class == \"LV\" and other_class == \"HV\" require clearance >= 0.8mm then error"
    ));
    assert_eq!(undo_steps(&app, id), 1);
    let tab = &app.document_state.project_tabs[&id].rules;
    assert!(tab.matrix_edit.is_none());
    assert_eq!(tab.matrix_open, Some(("LV".into(), "HV".into())));
    let _ = app.view_rules_tab(id);

    command(&mut app, "undo");
    assert_eq!(dsl(&app), TEXT);
    command(&mut app, "redo");
    assert!(dsl(&app).contains("cc_LV_HV"));
}

#[test]
fn a_sub_grid_cell_narrows_to_an_object_pair_and_clear_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    let key = CellKey::new("HV", "LV").with_pair("track-pad").unwrap();
    send(
        &mut app,
        RulesTabMsg::MatrixCellOpened {
            project_id: id,
            key: key.clone(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellEdited {
            project_id: id,
            text: "1mm".into(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellCommitted { project_id: id },
    );
    assert!(
        dsl(&app).contains("rule cc_HV_LV_track_pad on clearance between track and pad"),
        "{}",
        dsl(&app)
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellCleared {
            project_id: id,
            key,
        },
    );
    assert_eq!(dsl(&app), TEXT);
    assert_eq!(undo_steps(&app, id), 2);
}

#[test]
fn a_bad_matrix_value_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::MatrixCellOpened {
            project_id: id,
            key: CellKey::new("HV", "LV"),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellEdited {
            project_id: id,
            text: "wide".into(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellCommitted { project_id: id },
    );
    let tab = &app.document_state.project_tabs[&id].rules;
    assert!(tab.refused.is_some());
    assert!(tab.matrix_edit.is_some(), "the cell stays open");
    assert_eq!(dsl(&app), TEXT);
    assert_eq!(undo_steps(&app, id), 0);
}

#[test]
fn the_matrix_commands_read_and_set_cells_with_json() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    let set = answer(
        &mut app,
        "rules_matrix_set",
        &["class_a=HV", "class_b=LV", "value=0.8mm"],
    );
    assert_eq!(set["change"], "added");
    assert_eq!(set["rule"], "cc_HV_LV");
    assert_eq!(set["cell"]["value"], "0.8mm");
    assert_eq!(undo_steps(&app, id), 1);

    let got = answer(&mut app, "rules_matrix_get", &[]);
    assert_eq!(got["classes"], serde_json::json!(["HV", "LV"]));
    assert_eq!(got["cells"][0]["rule"], "cc_HV_LV");
    assert_eq!(got["defaults_from_board"], true);
    let fallback = got["defaults"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["class_a"] == "HV" && d["class_b"] == "LV" && d["object_pair"].is_null())
        .map(|d| d["value_mm"].clone());
    assert_eq!(fallback, Some(serde_json::json!(0.2)));

    // No value clears the cell.
    let cleared = answer(&mut app, "rules_matrix_set", &["class_a=LV", "class_b=HV"]);
    assert_eq!(cleared["change"], "removed");
    assert_eq!(dsl(&app), TEXT);

    let unknown = run(
        &mut app,
        "rules_matrix_set",
        &["class_a=HV", "class_b=Nope", "value=1mm"],
    );
    assert!(
        matches!(&unknown, CommandOutcome::Disabled { reason } if reason.contains("no net class 'Nope'")),
        "{unknown:?}"
    );
}

#[test]
fn a_rule_set_is_created_applied_and_redefined_from_the_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::PanelPicked {
            project_id: id,
            panel: Some(RulesPanel::Sets),
        },
    );
    type_in(&mut app, id, SetFormField::Name, "HV_08");
    type_in(
        &mut app,
        id,
        SetFormField::Values,
        "min_clearance=0.8mm, min_width=0.3mm",
    );
    send(&mut app, RulesTabMsg::SetCreated { project_id: id });
    assert!(drc(&app).sets.contains_key("HV_08"));
    assert_eq!(dsl(&app), TEXT, "a set alone writes no rule");
    let _ = app.view_rules_tab(id);

    type_in(&mut app, id, SetFormField::Scope, "class == \"HV\"");
    send(
        &mut app,
        RulesTabMsg::SetApplied {
            project_id: id,
            set: "HV_08".into(),
        },
    );
    let text = dsl(&app);
    assert!(text.starts_with(TEXT), "{text}");
    assert!(text.contains("rule HV_08_HV_clearance on clearance when class == \"HV\" require clearance >= 0.8mm then error"));
    assert!(text.contains(
        "rule HV_08_HV_track on track when class == \"HV\" require width >= 0.3mm then error"
    ));
    assert_eq!(drc(&app).sets["HV_08"].uses.len(), 1);

    // New limits: every rule the set wrote follows.
    type_in(&mut app, id, SetFormField::Values, "min_clearance=1mm");
    send(
        &mut app,
        RulesTabMsg::SetRedefined {
            project_id: id,
            set: "HV_08".into(),
        },
    );
    let text = dsl(&app);
    assert!(text.contains("rule HV_08_HV_clearance on clearance when class == \"HV\" require clearance >= 1mm then error"));
    assert!(!text.contains("HV_08_HV_track"), "{text}");
    assert_eq!(undo_steps(&app, id), 3);
    let _ = app.view_rules_tab(id);

    command(&mut app, "undo");
    assert!(dsl(&app).contains("HV_08_HV_track"));
    assert_eq!(
        drc(&app).sets["HV_08"]
            .limits
            .get("min_width")
            .map(String::as_str),
        Some("0.3mm"),
        "the set and its rules step back together"
    );
}

#[test]
fn the_rule_set_commands_answer_with_the_set() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(&dir);
    let created = answer(
        &mut app,
        "rules_set_create",
        &["name=hv", "values=min_clearance=0.8mm", "severity=warning"],
    );
    assert_eq!(created["set"]["limits"]["min_clearance"], "0.8mm");
    assert_eq!(created["set"]["severity"], "warning");
    let applied = answer(
        &mut app,
        "rules_set_apply",
        &["set=hv", "scope=net == \"LV_A\""],
    );
    assert_eq!(applied["rules"], serde_json::json!(["hv_LV_A_clearance"]));
    let edited = answer(
        &mut app,
        "rules_set_edit",
        &["set=hv", "values=min_clearance=0.9mm"],
    );
    assert_eq!(edited["set"]["uses"][0]["rules"][0], "hv_LV_A_clearance");
    // No severity keeps the set's.
    assert!(dsl(&app).contains("require clearance >= 0.9mm then warning"));
    let listed = answer(&mut app, "rules_list", &[]);
    assert_eq!(listed["sets"][0]["name"], "hv");
    let twice = run(
        &mut app,
        "rules_set_create",
        &["name=hv", "values=min_clearance=1mm"],
    );
    assert!(
        matches!(twice, CommandOutcome::Disabled { .. }),
        "{twice:?}"
    );
}

/// A set's new severity governs its rules: a severity entry for a rule
/// whose `then` word the set changed is dropped in the same step, and an
/// entry for a rule the set left alone stays.
#[test]
fn a_set_severity_change_drops_the_stale_entry_of_its_rules() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    let _ = answer(
        &mut app,
        "rules_set_create",
        &["name=hv", "values=min_clearance=0.8mm"],
    );
    let _ = answer(
        &mut app,
        "rules_set_apply",
        &["set=hv", "scope=net == \"LV_A\""],
    );
    let severity = &mut app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .expect("stored rules")
        .drc
        .severity;
    severity.insert("user::hv_LV_A_clearance".into(), "info".into());
    severity.insert("user::base".into(), "warning".into());

    let steps = undo_steps(&app, id);
    let _ = answer(
        &mut app,
        "rules_set_edit",
        &["set=hv", "values=min_clearance=0.8mm", "severity=warning"],
    );
    assert!(dsl(&app).contains("require clearance >= 0.8mm then warning"));
    let entries = drc(&app).severity;
    assert!(
        !entries.contains_key("user::hv_LV_A_clearance"),
        "{entries:?}"
    );
    assert_eq!(
        entries.get("user::base").map(String::as_str),
        Some("warning")
    );
    assert_eq!(undo_steps(&app, id), steps + 1);
}

/// A rule a set wrote, edited by hand into one the table cannot show, is
/// not rewritten over: the set edit is refused and nothing changes.
#[test]
fn a_set_edit_over_a_hand_edited_rule_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    let _ = answer(
        &mut app,
        "rules_set_create",
        &["name=hv", "values=min_clearance=0.8mm"],
    );
    let _ = answer(
        &mut app,
        "rules_set_apply",
        &["set=hv", "scope=net == \"LV_A\""],
    );
    let typed = dsl(&app).replace(
        "when net == \"LV_A\"",
        "when net == \"LV_A\" and layer == TopCopper and layer == BottomCopper",
    );
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .expect("stored rules")
        .drc
        .dsl = Some(typed.clone());
    let steps = undo_steps(&app, id);
    let refused = run(
        &mut app,
        "rules_set_edit",
        &["set=hv", "values=min_clearance=1mm"],
    );
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if reason.contains("hv_LV_A_clearance")),
        "{refused:?}"
    );
    assert_eq!(dsl(&app), typed);
    assert_eq!(undo_steps(&app, id), steps);
}

#[test]
fn a_tier_preset_shows_its_diff_then_applies_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::PanelPicked {
            project_id: id,
            panel: Some(RulesPanel::Presets),
        },
    );
    send(
        &mut app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:standard".into(),
        },
    );
    let preview = app.document_state.project_tabs[&id]
        .rules
        .preview
        .clone()
        .expect("a preview");
    let names: Vec<&str> = preview.changes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "fab_clearance",
            "fab_track",
            "fab_via",
            "fab_hole",
            "fab_edge",
            "fab_mask",
            "fab_silk"
        ]
    );
    // #2311: a tier says where its numbers come from and names no maker.
    let fab = preview.fab.as_ref().expect("a tier names its source");
    assert!(fab.manufacturer.is_empty() && !fab.published.is_empty());
    assert_eq!(dsl(&app), TEXT, "the preview changes nothing");
    let _ = app.view_rules_tab(id);

    send(&mut app, RulesTabMsg::PresetApplied { project_id: id });
    let text = dsl(&app);
    assert!(text.starts_with(TEXT), "a tier merges: {text}");
    assert!(
        text.contains(
            "rule fab_via on via require diameter >= 0.5mm and drill >= 0.25mm then error"
        )
    );
    assert!(app.document_state.project_tabs[&id].rules.preview.is_none());
    assert_eq!(undo_steps(&app, id), 1);
    command(&mut app, "undo");
    assert_eq!(dsl(&app), TEXT);
}

#[test]
fn a_preview_of_rules_that_moved_since_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    send(
        &mut app,
        RulesTabMsg::PresetPicked {
            project_id: id,
            preset: "tier:hdi".into(),
        },
    );
    let _ = answer(
        &mut app,
        "rules_matrix_set",
        &["class_a=HV", "class_b=LV", "value=0.8mm"],
    );
    send(&mut app, RulesTabMsg::PresetApplied { project_id: id });
    let tab = &app.document_state.project_tabs[&id].rules;
    assert!(
        tab.refused
            .as_deref()
            .is_some_and(|r| r.contains("changed since"))
    );
    assert!(!dsl(&app).contains("fab_"));
}

#[test]
fn the_preset_commands_diff_and_apply_tiers_and_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(&dir);
    let diff = answer(&mut app, "rules_preset_diff", &["preset=advanced"]);
    assert_eq!(diff["mode"], "merge");
    assert_eq!(diff["added"], 7);
    assert_eq!(diff["fab"]["published"], "2026-11-19", "{diff}");
    // An empty field is left out, as in the file: a tier names no maker.
    assert!(diff["fab"].get("manufacturer").is_none(), "{diff}");
    assert_eq!(diff["removed"], 0);
    assert_eq!(dsl(&app), TEXT);

    // A profile preset before the library was read is refused while read.
    let early = run(&mut app, "rules_preset_diff", &["preset=profile:Fab"]);
    assert!(
        matches!(early, CommandOutcome::Disabled { .. }),
        "{early:?}"
    );

    let profile = RuleProfile {
        schema: None,
        name: "Fab".into(),
        version: "1".into(),
        erc: Default::default(),
        drc: DrcRuleSet {
            dsl: Some("rule fab_only on track require width >= 0.2mm then error\n".into()),
            ..DrcRuleSet::default()
        },
        fab: None,
        unknown: Default::default(),
    };
    let manager = &mut app.ui_state.profile_manager;
    manager
        .listing
        .profiles
        .push(crate::rule_profiles::ListedProfile {
            profile,
            kind: crate::rule_profiles::ProfileKind::User,
        });
    manager.loaded = true;
    let diff = answer(&mut app, "rules_preset_diff", &["preset=profile:Fab"]);
    assert_eq!(diff["mode"], "replace");
    assert_eq!(diff["removed"], 1);
    assert_eq!(diff["changes"][0]["rule"], "base");
    assert_eq!(diff["changes"][0]["change"], "removed");
    let applied = answer(&mut app, "rules_preset_apply", &["preset=Fab"]);
    assert_eq!(applied["added"], 1);
    assert_eq!(
        dsl(&app),
        "rule fab_only on track require width >= 0.2mm then error\n"
    );
    let unknown = run(&mut app, "rules_preset_apply", &["preset=tier:nope"]);
    assert!(matches!(unknown, CommandOutcome::Disabled { .. }));
}

/// A tier preset sets the project's router tool in its rules' undo step:
/// one undo restores both and the project reads clean again; redo sets
/// both again (Caner 2026-11-20).
#[test]
fn undoing_a_tier_preset_restores_the_router_tool_with_the_rules() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    open(&mut app);
    let path = project_path(&app);
    app.document_state.mark_documents_clean_where(|_| true);
    app.document_state.note_project_clean_state(id);
    let router = |app: &Signex| app.document_state.projects[0].data.router_tool_diameter();
    assert_eq!(router(&app), None);

    let _ = answer(&mut app, "rules_preset_apply", &["preset=tier:hdi"]);
    assert_eq!(router(&app), Some(1.0));
    assert!(dsl(&app).contains("fab_via"));
    assert_eq!(undo_steps(&app, id), 1, "rules and router are one step");
    assert!(app.document_state.is_document_dirty(&path));

    command(&mut app, "undo");
    assert_eq!(router(&app), None, "the undo restores the router tool");
    assert_eq!(dsl(&app), TEXT);
    assert!(
        !app.document_state.is_document_dirty(&path),
        "back at the saved bytes"
    );

    command(&mut app, "redo");
    assert_eq!(router(&app), Some(1.0));
    assert!(dsl(&app).contains("fab_via"));
}

/// The New PCB tier seed is one step too: its undo drops the seeded rules
/// and the router tool together.
#[test]
fn undoing_a_seeded_tier_drops_its_router_tool_too() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    app.seed_tier_rules(id, signex_types::project::FabTemplate::Advanced);
    let router = |app: &Signex| app.document_state.projects[0].data.router_tool_diameter();
    assert_eq!(router(&app), Some(1.6));
    assert_eq!(undo_steps(&app, id), 1);

    assert!(app.step_rules_of(id, true));
    assert_eq!(router(&app), None);
    assert_eq!(dsl(&app), TEXT);
}
