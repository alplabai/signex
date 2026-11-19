//! #34 (Graphite mockup m34): the ERC and annotation section's rule source
//! pill, its severity tiles, Store ERC rules in this project and Open
//! Electrical Rules Check….

use signex_erc::Severity;
use signex_i18n::ids;

use super::ErcSource;
use super::view::{open_erc_message, source_pill, tile_words};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::styles::PillTone;

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-erc-look-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn source(app: &Signex) -> ErcSource {
    app.erc_section_facts(&app.document_state.projects[0])
        .source
}

#[test]
fn the_rule_source_pill_names_where_the_rules_come_from() {
    assert_eq!(
        source_pill(ErcSource::Machine),
        (ids::PROJECT_ERC_SOURCE_MACHINE, PillTone::Neutral)
    );
    assert_eq!(
        source_pill(ErcSource::Project),
        (ids::PROJECT_ERC_SOURCE_PROJECT, PillTone::Ok)
    );
    assert_eq!(
        source_pill(ErcSource::Unreadable),
        (ids::PROJECT_ERC_SOURCE_UNREADABLE, PillTone::Warn)
    );
}

#[test]
fn each_severity_tile_reads_as_a_rule_count() {
    let words = [
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ]
    .map(tile_words);
    assert_eq!(
        words,
        [
            ids::PROJECT_ERC_TILE_ERROR,
            ids::PROJECT_ERC_TILE_WARNING,
            ids::PROJECT_ERC_TILE_INFO,
            ids::PROJECT_ERC_TILE_OFF,
        ]
    );
}

#[test]
fn the_tiles_count_every_built_in_rule_once() {
    let (app, _) = app();
    let facts = app.erc_section_facts(&app.document_state.projects[0]);
    assert_eq!(facts.total(), signex_erc::RuleKind::ALL.len());
    let severities = facts.counts.map(|(severity, _)| severity);
    assert_eq!(
        severities,
        [
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Off
        ],
        "the tiles run Error, Warning, Info, Off"
    );
}

/// The Store button on the machine-rules line stores the rules as one step
/// of the rule history, and the pill follows.
#[test]
fn the_store_button_is_one_rule_history_step_and_the_pill_follows() {
    let (mut app, id) = app();
    assert_eq!(source(&app), ErcSource::Machine);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::ErcRulesStoreRequested { project_id: id },
    )));
    assert_eq!(source(&app), ErcSource::Project);
    assert_eq!(source_pill(source(&app)).1, PillTone::Ok);
    assert!(app.undo_project_rules());
    assert_eq!(source(&app), ErcSource::Machine);
    assert!(!app.undo_project_rules(), "storing was a single step");
}

/// Open Electrical Rules Check… opens the ERC dialog over this project: the
/// Project tab makes its project the one the dialog edits.
#[test]
fn open_electrical_rules_check_opens_the_dialog_for_this_project() {
    let (mut app, id) = app();
    app.ui_state.erc_dialog_open = false;
    let _ = app.update(open_erc_message());
    assert!(app.ui_state.erc_dialog_open);
    assert_eq!(
        app.document_state
            .active_document_project()
            .map(|project| project.id),
        Some(id)
    );
}

/// With two projects loaded and the second one's tab in front, the dialog
/// edits the second project, not the first.
#[test]
fn open_electrical_rules_check_from_a_second_project_edits_that_project() {
    let (mut app, first) = app();
    let mut second = app.document_state.projects[0].clone();
    let dir = std::env::temp_dir().join(format!("signex-prj-erc-two-{}", uuid::Uuid::now_v7()));
    second.id = app.document_state.mint_project_id();
    second.path = dir.join("ProjectB.snxprj");
    second.data.name = "ProjectB".to_string();
    second.data.dir = dir.to_string_lossy().into_owned();
    let second_id = second.id;
    app.document_state.projects.push(second);
    app.open_project_tab(1);
    app.ui_state.erc_dialog_open = false;
    let _ = app.update(open_erc_message());
    assert!(app.ui_state.erc_dialog_open);
    let edited = app
        .document_state
        .active_document_project()
        .map(|project| project.id);
    assert_eq!(edited, Some(second_id));
    assert_ne!(edited, Some(first));
}

#[test]
fn the_section_renders_for_every_rule_source() {
    let (mut app, id) = app();
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: crate::app::ProjectSection::ErcAnnotation,
        },
    )));
    let _ = app.view_project_tab(id);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::ErcRulesStoreRequested { project_id: id },
    )));
    let _ = app.view_project_tab(id);
}
