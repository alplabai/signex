//! #2398 steps 5 and 6 - the class matrix, named rule sets and presets,
//! driven through `Signex::update` (the tab) and the command dispatch (the
//! agent): each edit is one undo step on the project's rule history and
//! rewrites only the rules it owns; the commands answer with JSON.

use std::path::PathBuf;

use serde_json::Value;
use signex_drc_dsl::table::matrix::CellKey;
use signex_types::net_class::NetClass;
use signex_types::pcb::{NetDef, PcbBoard, Point, Segment};
use signex_types::project::{DrcRuleSet, FabTemplate, ProjectRules, RuleProfile};
use signex_widgets::constraint_grid::{ClassPair, MatrixMessage};
use uuid::Uuid;

use super::matrix::Shown;
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

pub(super) fn app_with_board(dir: &tempfile::TempDir) -> (Signex, ProjectId) {
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

pub(super) fn dsl(app: &Signex) -> String {
    drc(app).dsl.unwrap_or_default()
}

pub(super) fn send(app: &mut Signex, msg: RulesTabMsg) {
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

pub(super) fn undo_steps(app: &Signex, id: ProjectId) -> usize {
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
    let analysis = app.document_view.project_tabs[&id]
        .rules
        .analysis
        .as_ref()
        .expect("the tab in view is analysed");
    let fallback = analysis
        .defaults
        .get(&("HV".to_string(), "LV".to_string(), None))
        .copied();
    assert_eq!(fallback, Some(200_000));

    // LV is class 1 and HV class 0: (1, 0) is the live LV-HV cell.
    act(
        &mut app,
        id,
        MatrixMessage::CellPressed(ClassPair { row: 1, col: 0 }),
    );
    let _ = app.view_rules_tab(id);
    act(&mut app, id, MatrixMessage::EditTextChanged("0.8mm".into()));
    act(&mut app, id, MatrixMessage::EditSubmitted);
    let text = dsl(&app);
    assert!(text.starts_with(TEXT), "the other rule is as typed: {text}");
    assert!(text.contains(
        "rule cc_LV_HV on clearance when class == \"LV\" and other_class == \"HV\" require clearance >= 0.8mm then error"
    ));
    assert_eq!(undo_steps(&app, id), 1);
    let tab = &app.document_view.project_tabs[&id].rules;
    assert!(tab.matrix_edit.is_none());
    assert!(tab.matrix.edit().is_none(), "the cell closed");
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
        RulesTabMsg::MatrixPairTyped {
            project_id: id,
            key: key.clone(),
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
        RulesTabMsg::MatrixPairTyped {
            project_id: id,
            key: CellKey::new("HV", "LV"),
            text: "wide".into(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixCellCommitted { project_id: id },
    );
    let tab = &app.document_view.project_tabs[&id].rules;
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
        matches!(&unknown, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("no net class 'Nope'")),
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
        matches!(&refused, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("hv_LV_A_clearance")),
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
    let preview = app.document_view.project_tabs[&id]
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
    assert!(app.document_view.project_tabs[&id].rules.preview.is_none());
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
    let tab = &app.document_view.project_tabs[&id].rules;
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

/// What the matrix shows at `key`, from the rules as stored now.
pub(super) fn shown_at(app: &Signex, id: ProjectId, key: &CellKey) -> Option<Shown> {
    let table = signex_drc_dsl::table::table(&dsl(app));
    super::matrix::shown(&table, &app.rules_matrix_defaults(id), key)
}

pub(super) fn show_matrix(app: &mut Signex, id: ProjectId) {
    open(app);
    send(
        app,
        RulesTabMsg::ClearanceViewPicked {
            project_id: id,
            view: ClearanceView::Matrix,
        },
    );
    let _ = app.view_rules_tab(id);
}

fn act(app: &mut Signex, id: ProjectId, msg: MatrixMessage) {
    send(
        app,
        RulesTabMsg::MatrixActed {
            project_id: id,
            msg,
        },
    );
}

#[test]
fn only_the_lower_triangle_of_the_matrix_is_edited() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    // HV is class 0 and LV class 1: (0, 1) is the mirror half, folded onto
    // the live cell (1, 0) before the widget sees it.
    let mirror = ClassPair { row: 0, col: 1 };
    act(&mut app, id, MatrixMessage::CellPressed(mirror));
    let tab = &app.document_view.project_tabs[&id].rules;
    assert_eq!(tab.matrix.pinned(), Some(ClassPair { row: 1, col: 0 }));
    assert_eq!(tab.matrix_open, Some(("LV".into(), "HV".into())));
    assert_eq!(tab.matrix_pin, Some(CellKey::new("LV", "HV")));
    let _ = app.view_rules_tab(id);

    act(&mut app, id, MatrixMessage::EditTextChanged("0.8".into()));
    act(&mut app, id, MatrixMessage::EditSubmitted);
    let text = dsl(&app);
    assert!(text.starts_with(TEXT), "the other rule is as typed: {text}");
    assert!(
        text.contains(
            "rule cc_LV_HV on clearance when class == \"LV\" and other_class == \"HV\" require clearance >= 0.8mm then error"
        ),
        "{text}"
    );
    assert_eq!(text.matches("rule cc_").count(), 1, "one cell, one rule");
    assert_eq!(undo_steps(&app, id), 1);
    assert_eq!(
        shown_at(&app, id, &CellKey::new("HV", "LV")),
        Some(Shown::Written(0.8)),
        "both orders name the same cell"
    );
    let _ = app.view_rules_tab(id);

    // A clear on the mirror half clears the live cell, as one more step.
    act(&mut app, id, MatrixMessage::CellCleared(mirror));
    assert_eq!(dsl(&app), TEXT);
    assert_eq!(undo_steps(&app, id), 2);
}

#[test]
fn a_cleared_object_pair_falls_back_to_the_all_objects_value() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    let whole = CellKey::new("HV", "LV");
    let pair = CellKey::new("HV", "LV").with_pair("track-pad").unwrap();
    for (key, typed) in [(&whole, "0.8"), (&pair, "1")] {
        send(
            &mut app,
            RulesTabMsg::MatrixPairTyped {
                project_id: id,
                key: key.clone(),
                text: typed.into(),
            },
        );
        send(
            &mut app,
            RulesTabMsg::MatrixCellCommitted { project_id: id },
        );
    }
    assert!(dsl(&app).contains("between track and pad"), "{}", dsl(&app));
    assert_eq!(shown_at(&app, id, &pair), Some(Shown::Written(1.0)));
    // An object pair with no rule of its own shows the All objects value.
    let via = CellKey::new("HV", "LV").with_pair("via-via").unwrap();
    assert_eq!(shown_at(&app, id, &via), Some(Shown::Fallback(0.8)));
    let _ = app.view_rules_tab(id);

    send(
        &mut app,
        RulesTabMsg::MatrixCellCleared {
            project_id: id,
            key: pair.clone(),
        },
    );
    assert!(!dsl(&app).contains("track_pad"));
    assert_eq!(
        shown_at(&app, id, &pair),
        Some(Shown::Fallback(0.8)),
        "cleared, it shows the All objects value"
    );
    assert_eq!(undo_steps(&app, id), 3);
}

#[test]
fn a_value_typed_on_the_preview_writes_the_pinned_cell() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    // Nothing pinned: nothing is written.
    let _ = super::update_matrix::preview_committed(&mut app, id, 0.5);
    assert_eq!(dsl(&app), TEXT);

    act(
        &mut app,
        id,
        MatrixMessage::CellPressed(ClassPair { row: 1, col: 0 }),
    );
    let _ = super::update_matrix::preview_committed(&mut app, id, 0.6);
    let text = dsl(&app);
    assert!(text.contains("rule cc_LV_HV on clearance"), "{text}");
    assert!(text.contains("clearance >= 0.6mm"), "{text}");
    assert_eq!(undo_steps(&app, id), 1);

    // A clicked object-pair row takes the pin: the drawing writes it, not
    // the cell under the pointer.
    let pair = CellKey::new("LV", "HV").with_pair("track-via").unwrap();
    send(
        &mut app,
        RulesTabMsg::MatrixPairPinned {
            project_id: id,
            key: pair.clone(),
        },
    );
    send(
        &mut app,
        RulesTabMsg::MatrixHovered {
            project_id: id,
            key: Some(CellKey::new("LV", "LV")),
        },
    );
    let _ = app.view_rules_tab(id);
    let _ = super::update_matrix::preview_committed(&mut app, id, 0.45);
    assert_eq!(shown_at(&app, id, &pair), Some(Shown::Written(0.45)));
    assert!(!dsl(&app).contains("cc_LV_LV"));
    assert_eq!(undo_steps(&app, id), 2);
    command(&mut app, "undo");
    assert_eq!(shown_at(&app, id, &pair), Some(Shown::Fallback(0.6)));
}

#[test]
fn the_tier_cards_show_the_canonical_tier_table() {
    for tier in FabTemplate::ALL {
        let spec = super::presets::tier_spec(tier);
        let table = tier.capabilities();
        assert_eq!(spec.clearance, table.clearance, "{tier:?}");
        assert_eq!(spec.width, table.track_width, "{tier:?}");
        assert_eq!(spec.drill, table.hole, "{tier:?}");
        assert_eq!(spec.annular, table.annular_ring, "{tier:?}");
    }
    let standard = super::presets::tier_spec(FabTemplate::Standard);
    assert_eq!(
        (
            standard.clearance,
            standard.width,
            standard.drill,
            standard.annular
        ),
        (0.15, 0.15, 0.25, 0.125)
    );
}

#[test]
fn a_tier_card_previews_old_and_new_values_and_applies_as_one_step() {
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
    let added = (None, Some("0.15mm".to_string()));
    let changed = (Some("0.15mm".to_string()), Some("0.1mm".to_string()));
    for (tier, steps, values) in [("tier:standard", 1, added), ("tier:advanced", 2, changed)] {
        send(
            &mut app,
            RulesTabMsg::PresetPicked {
                project_id: id,
                preset: tier.into(),
            },
        );
        let _ = app.view_rules_tab(id);
        let preview = app.document_view.project_tabs[&id]
            .rules
            .preview
            .clone()
            .expect("a preview");
        let clearance = preview
            .changes
            .iter()
            .find(|c| c.name == "fab_clearance")
            .expect("the clearance floor changes");
        assert_eq!(super::presets::change_values(clearance), values, "{tier}");
        send(&mut app, RulesTabMsg::PresetApplied { project_id: id });
        assert!(app.document_view.project_tabs[&id].rules.preview.is_none());
        assert_eq!(undo_steps(&app, id), steps, "one undo step per apply");
    }
    assert!(dsl(&app).contains("rule fab_clearance on clearance require clearance >= 0.1mm"));
}

#[test]
fn a_matrix_cell_stores_its_length_as_typed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    show_matrix(&mut app, id);
    let live = ClassPair { row: 1, col: 0 };
    let key = CellKey::new("LV", "HV");
    for (step, (typed, written, mm)) in
        [("8mil", "8mil", 0.2032), ("0.12345", "0.12345mm", 0.12345)]
            .into_iter()
            .enumerate()
    {
        act(&mut app, id, MatrixMessage::CellPressed(live));
        act(&mut app, id, MatrixMessage::EditTextChanged(typed.into()));
        act(&mut app, id, MatrixMessage::EditSubmitted);
        let text = dsl(&app);
        assert!(
            text.contains(&format!("clearance >= {written} then")),
            "{typed} is stored as typed: {text}"
        );
        assert_eq!(shown_at(&app, id, &key), Some(Shown::Written(mm)));
        assert_eq!(undo_steps(&app, id), step + 1);
        assert!(
            app.document_view.project_tabs[&id]
                .rules
                .matrix
                .edit()
                .is_none(),
            "the cell closed"
        );
        let _ = app.view_rules_tab(id);
    }
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

fn min_slot(app: &Signex) -> Option<f64> {
    app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|rules| rules.min_slot())
}

/// mockup row 179d: a tier preset stores its min slot with the rules, in
/// the same undo step as its rules and router tool.
#[test]
fn a_tier_preset_stores_its_min_slot_in_the_same_undo_step() {
    for (tier, expected) in [("standard", 0.8), ("advanced", 0.6), ("hdi", 0.5)] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, id) = app_with_board(&dir);
        open(&mut app);
        assert_eq!(min_slot(&app), None);

        let preset = format!("preset=tier:{tier}");
        let _ = answer(&mut app, "rules_preset_apply", &[preset.as_str()]);
        assert_eq!(min_slot(&app), Some(expected), "{tier}");
        assert_eq!(undo_steps(&app, id), 1, "rules and min slot are one step");

        command(&mut app, "undo");
        assert_eq!(min_slot(&app), None, "the undo restores the min slot");
        command(&mut app, "redo");
        assert_eq!(min_slot(&app), Some(expected), "{tier}");
    }
}

/// The New PCB tier seed stores the tier's min slot too; its undo drops it.
#[test]
fn a_seeded_tier_stores_its_min_slot_and_undo_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_board(&dir);
    app.seed_tier_rules(id, signex_types::project::FabTemplate::Hdi);
    assert_eq!(min_slot(&app), Some(0.5));
    assert_eq!(undo_steps(&app, id), 1);
    assert!(app.step_rules_of(id, true));
    assert_eq!(min_slot(&app), None);
}
