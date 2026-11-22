//! m84 ERC dialog, m85 ERC panel, m86 Messages, m87 DRC panel, m88 DRC results.

use std::sync::Arc;

use iced::widget::text_editor::{Action, Edit};
use signex_app::app::{ErcMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{
    DrcCategory, DrcFilterChoice, DrcPanelMsg, ErcPanelMsg, ErcSeverityLite, ErcWaiverMsg,
    MessageLevel, MessagesPanelMsg, PanelMsg,
};
use signex_erc::waivers::violation::WaiverKey;
use signex_erc::{RuleKind, Severity};
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::violation::Severity as DrcSeverity;
use uuid::Uuid;

use super::{BoardRig, Rig, board_of, empty_board, open_board, part, rig, sheet, undo};
use crate::settle_support::settle;

const FIRST: Uuid = Uuid::from_u128(0x8401);
const SECOND: Uuid = Uuid::from_u128(0x8402);
const DUPLICATE: RuleKind = RuleKind::DuplicateRefDesignator;

fn erc(app: &mut Signex, msg: ErcMsg) {
    settle(app, Message::Erc(msg));
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    settle(app, Message::Dock(DockMessage::Panel(msg)));
}

/// One sheet with two parts that share the designator R1. The sheet embeds
/// their library body: ERC does not see a part without one.
fn duplicate_rig() -> Rig {
    let mut content = sheet(
        vec![
            part(FIRST, "R1", "10k", 0.0),
            part(SECOND, "R1", "10k", 20.0),
        ],
        Vec::new(),
    );
    let body: signex_types::schematic::LibSymbol =
        serde_json::from_str(r#"{"id":"Device:R"}"#).expect("a library body");
    content.lib_symbols.insert(body.id.clone(), body);
    rig(content, &[], None)
}

fn stored_severity(rig: &Rig) -> Option<Severity> {
    let project = rig.app.document_state.project_by_id(rig.project)?;
    let rules = project.data.rules.as_ref()?;
    signex_erc::project_rules::resolve(&rules.erc)
        .severity
        .get(&DUPLICATE)
        .copied()
}

fn stored_pin_cell(rig: &Rig) -> Option<Severity> {
    let project = rig.app.document_state.project_by_id(rig.project)?;
    let rules = project.data.rules.as_ref()?;
    signex_erc::project_rules::resolve(&rules.erc)
        .pin_matrix
        .get(&(0, 0))
        .copied()
}

fn waiver_count(rig: &Rig) -> usize {
    rig.app
        .document_state
        .project_by_id(rig.project)
        .and_then(|project| project.data.rules.as_ref())
        .map_or(0, |rules| rules.erc_waivers.len())
}

fn duplicate_findings(app: &Signex) -> usize {
    app.ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .filter(|violation| violation.rule == DUPLICATE)
        .count()
}

fn duplicate_key(app: &Signex) -> WaiverKey {
    app.ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .find(|violation| violation.rule == DUPLICATE)
        .and_then(|violation| violation.waiver_key.clone())
        .expect("the duplicate finding can be waived")
}

fn paste(text: &str) -> Action {
    Action::Edit(Edit::Paste(Arc::new(text.to_string())))
}

// ---- m84 ERC dialog ----

/// The dialog open with the rules stored in the project, so no edit reaches
/// this machine's profile.
fn erc_dialog() -> Rig {
    let mut rig = duplicate_rig();
    erc(&mut rig.app, ErcMsg::OpenDialog);
    assert!(rig.app.ui_state.erc_dialog_open);
    erc(&mut rig.app, ErcMsg::StoreRulesInProject);
    assert!(
        rig.app
            .document_state
            .project_by_id(rig.project)
            .is_some_and(|project| project.data.rules.is_some()),
        "the project stores its rules"
    );
    rig
}

#[test]
fn m84_a_severity_edit_changes_the_project_rules_and_marks_the_project_unsaved() {
    let mut rig = erc_dialog();

    erc(
        &mut rig.app,
        ErcMsg::SeverityChanged(DUPLICATE, Severity::Warning),
    );

    assert_eq!(stored_severity(&rig), Some(Severity::Warning));
    assert!(rig.app.document_state.is_document_dirty(&rig.prj));
}

#[test]
fn m84_rule_undo_and_redo_step_a_severity_edit() {
    let mut rig = erc_dialog();
    let before = stored_severity(&rig);
    erc(
        &mut rig.app,
        ErcMsg::SeverityChanged(DUPLICATE, Severity::Info),
    );

    erc(&mut rig.app, ErcMsg::RulesUndone);
    assert_eq!(stored_severity(&rig), before);

    erc(&mut rig.app, ErcMsg::RulesRedone);
    assert_eq!(stored_severity(&rig), Some(Severity::Info));
}

#[test]
fn m84_a_pin_matrix_click_cycles_the_cell_and_undo_restores_it() {
    let mut rig = erc_dialog();
    let before = stored_pin_cell(&rig);

    erc(&mut rig.app, ErcMsg::PinMatrixCellCycled { row: 0, col: 0 });
    assert_ne!(stored_pin_cell(&rig), before);

    erc(&mut rig.app, ErcMsg::RulesUndone);
    assert_eq!(stored_pin_cell(&rig), before);
}

#[test]
fn m84_run_erc_uses_the_severity_set_in_the_dialog() {
    let mut rig = erc_dialog();
    erc(
        &mut rig.app,
        ErcMsg::SeverityChanged(DUPLICATE, Severity::Warning),
    );

    erc(&mut rig.app, ErcMsg::Run);

    let finding = rig
        .app
        .ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .find(|violation| violation.rule == DUPLICATE)
        .expect("the duplicate is found");
    assert_eq!(finding.severity, Severity::Warning);
}

#[test]
fn m84_close_without_an_edit_stores_nothing_and_marks_nothing_unsaved() {
    let mut rig = duplicate_rig();
    erc(&mut rig.app, ErcMsg::OpenDialog);

    erc(&mut rig.app, ErcMsg::CloseDialog);

    assert!(!rig.app.ui_state.erc_dialog_open);
    assert_eq!(stored_severity(&rig), None);
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m84_save_as_profile_refuses_a_blank_name_and_cancel_closes_the_form() {
    let mut rig = erc_dialog();
    erc(&mut rig.app, ErcMsg::ProfileSaveDialogOpened);
    erc(
        &mut rig.app,
        ErcMsg::ProfileSaveNameChanged("   ".to_string()),
    );

    erc(&mut rig.app, ErcMsg::ProfileSaveSubmitted);

    let form = rig
        .app
        .ui_state
        .save_profile_dialog
        .as_ref()
        .expect("the form stays open");
    assert!(form.error.is_some(), "it says why");
    assert!(!form.busy, "nothing is being written");

    erc(&mut rig.app, ErcMsg::ProfileSaveCancelled);
    assert!(rig.app.ui_state.save_profile_dialog.is_none());
}

// ---- m85 ERC panel ----

fn ran_erc() -> Rig {
    let mut rig = duplicate_rig();
    erc(&mut rig.app, ErcMsg::Run);
    assert!(duplicate_findings(&rig.app) > 0, "the duplicate is listed");
    rig
}

fn start_waive(rig: &mut Rig) -> WaiverKey {
    let key = duplicate_key(&rig.app);
    let sheet = rig.root.clone();
    panel(
        &mut rig.app,
        PanelMsg::ErcWaiver(ErcWaiverMsg::Started {
            key: key.clone(),
            message: "R1 is used on 2 symbols".to_string(),
            sheet,
        }),
    );
    assert!(rig.app.ui_state.erc_waive_draft.is_some(), "the form opens");
    key
}

fn fill_waiver(rig: &mut Rig, reason: &str, author: &str) {
    panel(
        &mut rig.app,
        PanelMsg::ErcWaiver(ErcWaiverMsg::ReasonActed(paste(reason))),
    );
    panel(
        &mut rig.app,
        PanelMsg::ErcWaiver(ErcWaiverMsg::AuthorChanged(author.to_string())),
    );
}

#[test]
fn m85_a_waiver_with_reason_and_author_hides_the_finding_and_restore_brings_it_back() {
    let mut rig = ran_erc();
    let key = start_waive(&mut rig);
    fill_waiver(&mut rig, "Two footprints share one part", "A. Designer");

    panel(&mut rig.app, PanelMsg::ErcWaiver(ErcWaiverMsg::Confirmed));

    assert!(
        rig.app.ui_state.erc_waive_draft.is_none(),
        "the form closes"
    );
    assert_eq!(waiver_count(&rig), 1);
    assert!(!rig.app.ui_state.erc_waived_findings.is_empty());
    assert!(rig.app.document_state.is_document_dirty(&rig.prj));

    panel(
        &mut rig.app,
        PanelMsg::ErcWaiver(ErcWaiverMsg::Removed {
            rule_id: key.rule_id,
            objects: key.objects,
        }),
    );

    assert_eq!(waiver_count(&rig), 0);
    assert!(duplicate_findings(&rig.app) > 0, "the finding is back");
}

#[test]
fn m85_a_blank_reason_is_refused_and_the_form_stays_open() {
    let mut rig = ran_erc();
    start_waive(&mut rig);
    fill_waiver(&mut rig, "   ", "A. Designer");

    panel(&mut rig.app, PanelMsg::ErcWaiver(ErcWaiverMsg::Confirmed));

    let draft = rig
        .app
        .ui_state
        .erc_waive_draft
        .as_ref()
        .expect("the form stays open");
    assert!(draft.error.is_some(), "it says why");
    assert_eq!(waiver_count(&rig), 0);
}

#[test]
fn m85_a_blank_author_is_refused() {
    let mut rig = ran_erc();
    start_waive(&mut rig);
    fill_waiver(&mut rig, "Two footprints share one part", "  ");

    panel(&mut rig.app, PanelMsg::ErcWaiver(ErcWaiverMsg::Confirmed));

    assert!(rig.app.ui_state.erc_waive_draft.is_some(), "the form stays");
    assert_eq!(waiver_count(&rig), 0);
}

#[test]
fn m85_cancel_on_the_waive_form_stores_nothing() {
    let mut rig = ran_erc();
    start_waive(&mut rig);
    fill_waiver(&mut rig, "Two footprints share one part", "A. Designer");

    panel(&mut rig.app, PanelMsg::ErcWaiver(ErcWaiverMsg::Cancelled));

    assert!(rig.app.ui_state.erc_waive_draft.is_none());
    assert_eq!(waiver_count(&rig), 0);
    assert!(duplicate_findings(&rig.app) > 0);
}

#[test]
fn m85_rule_undo_takes_a_waiver_back() {
    let mut rig = ran_erc();
    start_waive(&mut rig);
    fill_waiver(&mut rig, "Two footprints share one part", "A. Designer");
    panel(&mut rig.app, PanelMsg::ErcWaiver(ErcWaiverMsg::Confirmed));
    assert_eq!(waiver_count(&rig), 1);

    erc(&mut rig.app, ErcMsg::RulesUndone);

    assert_eq!(waiver_count(&rig), 0);
    assert!(
        duplicate_findings(&rig.app) > 0,
        "the finding is listed again"
    );
}

#[test]
fn m85_a_severity_tile_filters_and_a_second_press_clears_it() {
    let mut rig = ran_erc();

    panel(
        &mut rig.app,
        PanelMsg::ErcView(ErcPanelMsg::TileToggled(ErcSeverityLite::Error)),
    );
    assert!(rig.app.ui_state.validation_panels.erc.only == Some(ErcSeverityLite::Error));

    panel(
        &mut rig.app,
        PanelMsg::ErcView(ErcPanelMsg::TileToggled(ErcSeverityLite::Error)),
    );
    assert!(rig.app.ui_state.validation_panels.erc.only.is_none());
}

#[test]
fn m85_a_rule_group_folds_and_unfolds() {
    let mut rig = ran_erc();
    let group = "Duplicate reference designator".to_string();

    panel(
        &mut rig.app,
        PanelMsg::ErcView(ErcPanelMsg::GroupToggled(group.clone())),
    );
    assert!(
        rig.app
            .ui_state
            .validation_panels
            .erc
            .collapsed
            .contains(&group)
    );

    panel(
        &mut rig.app,
        PanelMsg::ErcView(ErcPanelMsg::GroupToggled(group.clone())),
    );
    assert!(
        !rig.app
            .ui_state
            .validation_panels
            .erc
            .collapsed
            .contains(&group)
    );
}

#[test]
fn m85_clear_empties_the_list_and_leaves_the_sheet() {
    let mut rig = ran_erc();

    panel(&mut rig.app, PanelMsg::ClearErc);

    assert_eq!(duplicate_findings(&rig.app), 0);
    assert!(!rig.app.document_state.is_document_dirty(&rig.root));
}

// ---- m86 Messages ----

fn messages(app: &mut Signex, msg: MessagesPanelMsg) {
    panel(app, PanelMsg::MessagesView(msg));
}

#[test]
fn m86_a_level_chip_and_the_filter_text_set_what_the_panel_lists() {
    let mut rig = duplicate_rig();

    messages(
        &mut rig.app,
        MessagesPanelMsg::LevelPicked(MessageLevel::Warning),
    );
    messages(
        &mut rig.app,
        MessagesPanelMsg::QueryEdited("pin".to_string()),
    );

    let view = &rig.app.ui_state.validation_panels.messages;
    assert_eq!(view.level, MessageLevel::Warning);
    assert_eq!(view.query, "pin");
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m86_clear_hides_the_lines_so_far_and_the_log_keeps_them() {
    let mut rig = duplicate_rig();
    let line = format!("g10 messages line {}", Uuid::now_v7());
    signex_app::diagnostics::log_warning(&line);
    let logged = signex_app::diagnostics::recent_entries()
        .into_iter()
        .find(|entry| entry.message == line)
        .expect("the line is logged");
    messages(&mut rig.app, MessagesPanelMsg::QueryEdited(String::new()));

    messages(&mut rig.app, MessagesPanelMsg::Cleared);

    let cleared = rig.app.ui_state.validation_panels.messages.cleared_through;
    assert!(
        cleared.is_some_and(|id| id >= logged.id),
        "Clear hides the line: {cleared:?}"
    );
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|entry| entry.message == line),
        "the log still holds it"
    );
}

// ---- m87 and m88 DRC ----

fn track(n: u128, y: f64, net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Two tracks with a 0.15 mm gap, under the default 0.2 mm clearance.
fn close_tracks() -> PcbBoard {
    let mut board = empty_board();
    board.segments = vec![track(1, 0.0, 1), track(2, 0.4, 2)];
    board
}

fn drc(app: &mut Signex, msg: DrcPanelMsg) {
    panel(app, PanelMsg::Drc(msg));
}

fn ran_drc() -> (BoardRig, String) {
    let mut rig = open_board(close_tracks());
    panel(&mut rig.app, PanelMsg::RunDrc);
    let key = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .and_then(|drc| drc.rows.first())
        .map(|row| row.key.clone())
        .expect("the close pair is a finding");
    (rig, key)
}

fn drc_waivers(rig: &BoardRig) -> usize {
    board_of(&rig.app, &rig.path).options.drc_waivers.len()
}

fn fill_drc_waiver(rig: &mut BoardRig, key: &str, reason: &str, author: &str) {
    drc(&mut rig.app, DrcPanelMsg::WaiveStarted(key.to_string()));
    drc(&mut rig.app, DrcPanelMsg::ReasonActed(paste(reason)));
    drc(&mut rig.app, DrcPanelMsg::AuthorEdited(author.to_string()));
}

#[test]
fn m87_before_the_first_run_the_panel_is_idle_and_names_the_default_clearance() {
    let rig = open_board(close_tracks());
    let idle = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("a board shows the DRC panel");
    assert!(!idle.ran);
    assert!(idle.rows.is_empty());
    assert_eq!(idle.default_clearance_mm.as_deref(), Some("0.2"));
}

#[test]
fn m87_the_base_rules_list_folds_and_opens() {
    let mut rig = open_board(close_tracks());
    let before = rig.app.ui_state.validation_panels.drc.base_rules_closed;

    drc(&mut rig.app, DrcPanelMsg::BaseRulesToggled);
    assert_eq!(
        rig.app.ui_state.validation_panels.drc.base_rules_closed,
        !before
    );

    drc(&mut rig.app, DrcPanelMsg::BaseRulesToggled);
    assert_eq!(
        rig.app.ui_state.validation_panels.drc.base_rules_closed,
        before
    );
}

#[test]
fn m87_run_drc_reports_the_finding_and_does_not_edit_the_board() {
    let mut rig = open_board(close_tracks());
    let file = std::fs::read(&rig.path).expect("board file");

    panel(&mut rig.app, PanelMsg::RunDrc);

    let done = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("the panel");
    assert!(done.ran && !done.running && !done.stale);
    assert_eq!((done.clearance, done.short, done.unconnected), (1, 0, 0));
    assert_eq!(done.rows.len(), 1);
    assert!(!rig.app.document_state.is_document_dirty(&rig.path));
    assert_eq!(std::fs::read(&rig.path).expect("board file"), file);
}

#[test]
fn m87_run_drc_with_no_board_open_reports_nothing() {
    let mut app = super::fresh_app();

    panel(&mut app, PanelMsg::RunDrc);

    assert!(app.document_state.panel_ctx.pcb_drc.is_none());
}

#[test]
fn m88_a_tile_lists_its_category_and_clear_filter_lists_all() {
    let (mut rig, _key) = ran_drc();

    drc(&mut rig.app, DrcPanelMsg::TileToggled(DrcCategory::Short));
    assert!(matches!(
        rig.app.ui_state.validation_panels.drc.category,
        Some(DrcCategory::Short)
    ));
    let done = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    let row = done.rows.first().expect("a row");
    assert!(
        !rig.app.ui_state.validation_panels.drc.shows(row),
        "a clearance row is not under the Short tile"
    );

    drc(&mut rig.app, DrcPanelMsg::TileCleared);
    assert!(rig.app.ui_state.validation_panels.drc.category.is_none());
}

#[test]
fn m88_the_severity_filter_hides_rows_of_another_severity() {
    let (mut rig, _key) = ran_drc();

    drc(
        &mut rig.app,
        DrcPanelMsg::FilterPicked(DrcFilterChoice::Severity(Some(DrcSeverity::Warning))),
    );

    let done = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    let row = done.rows.first().expect("a row");
    assert!(
        !rig.app.interaction_state.pcb_drc.filter.shows(row),
        "the clearance error is not a warning"
    );

    drc(
        &mut rig.app,
        DrcPanelMsg::FilterPicked(DrcFilterChoice::Severity(None)),
    );
    let done = rig
        .app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    let row = done.rows.first().expect("a row");
    assert!(rig.app.interaction_state.pcb_drc.filter.shows(row));
}

#[test]
fn m88_a_waiver_is_stored_on_the_board_and_one_undo_takes_it_back() {
    let (mut rig, key) = ran_drc();
    fill_drc_waiver(&mut rig, &key, "Checked on the prototype", "A. Designer");

    drc(&mut rig.app, DrcPanelMsg::WaiveConfirmed);

    assert!(rig.app.interaction_state.pcb_drc.waive_draft.is_none());
    let waivers = &board_of(&rig.app, &rig.path).options.drc_waivers;
    assert_eq!(waivers.len(), 1);
    assert_eq!(waivers[0].key, key);
    assert_eq!(waivers[0].note, "Checked on the prototype");
    assert_eq!(waivers[0].author, "A. Designer");
    assert!(rig.app.document_state.is_document_dirty(&rig.path));

    undo(&mut rig.app);

    assert_eq!(drc_waivers(&rig), 0);
}

#[test]
fn m88_unwaive_removes_the_waiver() {
    let (mut rig, key) = ran_drc();
    fill_drc_waiver(&mut rig, &key, "Checked on the prototype", "A. Designer");
    drc(&mut rig.app, DrcPanelMsg::WaiveConfirmed);
    assert_eq!(drc_waivers(&rig), 1);

    drc(&mut rig.app, DrcPanelMsg::Unwaived(key));

    assert_eq!(drc_waivers(&rig), 0);
}

#[test]
fn m88_a_blank_reason_is_refused_and_the_form_stays_open() {
    let (mut rig, key) = ran_drc();
    fill_drc_waiver(&mut rig, &key, "   ", "A. Designer");

    drc(&mut rig.app, DrcPanelMsg::WaiveConfirmed);

    let draft = rig
        .app
        .interaction_state
        .pcb_drc
        .waive_draft
        .as_ref()
        .expect("the form stays open");
    assert!(draft.blank, "it marks the empty field");
    assert_eq!(drc_waivers(&rig), 0);
}

#[test]
fn m88_a_blank_author_is_refused() {
    let (mut rig, key) = ran_drc();
    fill_drc_waiver(&mut rig, &key, "Checked on the prototype", "  ");

    drc(&mut rig.app, DrcPanelMsg::WaiveConfirmed);

    assert!(rig.app.interaction_state.pcb_drc.waive_draft.is_some());
    assert_eq!(drc_waivers(&rig), 0);
}

#[test]
fn m88_cancel_on_the_waive_form_stores_nothing() {
    let (mut rig, key) = ran_drc();
    fill_drc_waiver(&mut rig, &key, "Checked on the prototype", "A. Designer");

    drc(&mut rig.app, DrcPanelMsg::WaiveCancelled);

    assert!(rig.app.interaction_state.pcb_drc.waive_draft.is_none());
    assert_eq!(drc_waivers(&rig), 0);
    assert!(!rig.app.document_state.is_document_dirty(&rig.path));
}
