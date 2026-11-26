//! Mockup row 32: the Variant Manager is a document tab of its own. The
//! Variants row opens it and a second open focuses it; a matrix cell is
//! selected by a click and edited in the right pane; Duplicate copies a
//! variant with the fits its sheets give it; Set as default moves the star.
//! Every edit is one undo step of the project's General history.

use signex_types::project::{FOOTPRINT_PARAMETER, VALUE_PARAMETER};
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::manager::{VariantCell, VariantManagerMsg};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex, TabKind};

fn part(reference: &str, value: &str, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Default::default()
    }
}

/// R1 (fitted everywhere, 1% tolerance) and R2 (not fitted in Lite on its
/// sheet); Lite is defined. The folder lives as long as the guard.
pub(super) fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let sheet = SchematicSheet {
        symbols: vec![
            part("R1", "1k", &[("Tolerance", "1%")]),
            part("R2", "10k", &[("Fitted@Lite", "no")]),
        ],
        ..Default::default()
    };
    open_with(&mut app.document_state, &dir.path().join("a.snxsch"), sheet);
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".to_string()];
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(msg)))
}

pub(super) fn manage(
    app: &mut Signex,
    id: ProjectId,
    msg: VariantManagerMsg,
) -> iced::Task<Message> {
    send(
        app,
        ProjectTabMsg::VariantManager {
            project_id: id,
            msg,
        },
    )
}

pub(super) fn settle(app: &mut Signex, task: iced::Task<Message>) {
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

/// The Variants row of the Project tab's list, as a click sends it.
pub(super) fn open_manager(app: &mut Signex, id: ProjectId) {
    let task = send(
        app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Variants,
        },
    );
    settle(app, task);
}

pub(super) fn cell(variant: Option<&str>, designator: &str) -> VariantCell {
    VariantCell {
        variant: variant.map(str::to_string),
        designator: designator.to_string(),
    }
}

fn manager_tabs(app: &Signex, id: ProjectId) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|tab| tab.kind.as_variant_manager() == Some(id))
        .count()
}

pub(super) fn data(app: &Signex) -> &signex_types::project::ProjectData {
    &app.document_state.projects[0].data
}

/// The read with the settings laid over it, as the views show it.
fn shown(app: &Signex, id: ProjectId) -> signex_output::VariantMatrix {
    let read = app.document_view.project_tabs[&id]
        .variant_parts
        .as_deref()
        .expect("the parts were read")
        .as_ref()
        .expect("the parts were read without a problem");
    read.with_settings(&data(app).variant_settings)
}

#[test]
fn the_variants_row_opens_the_manager_tab_and_a_second_open_focuses_it() {
    let (mut app, id, _dir) = app();
    app.open_project_tab(0);
    let project_tab = app.document_state.active_tab;
    open_manager(&mut app, id);
    let active = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(active.kind, TabKind::VariantManager(p) if p == id));
    assert!(active.title.ends_with("Variants"), "{}", active.title);
    assert_eq!(active.path, app.document_state.projects[0].path);
    assert_eq!(
        app.document_view.project_tabs[&id].section,
        ProjectSection::General,
        "the Project tab stays on its section"
    );
    assert!(app.document_view.project_tabs[&id].variant_parts.is_some());

    // Back to the Project tab, then the tree's Variant Manager row: the same
    // tab comes to the front.
    app.document_state.active_tab = project_tab;
    let task = app.open_variant_manager(0);
    settle(&mut app, task);
    assert_eq!(manager_tabs(&app, id), 1);
    assert!(matches!(
        app.document_state.tabs[app.document_state.active_tab].kind,
        TabKind::VariantManager(p) if p == id
    ));
    let _ = app.view_variant_manager_tab(id);
}

#[test]
fn a_clicked_cell_is_edited_in_the_pane_and_each_pick_is_one_undo_step() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = cell(Some("Lite"), "R1");
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(r1.clone()));
    assert_eq!(
        app.document_view.project_tabs[&id].variant_manager.cell,
        Some(r1.clone())
    );
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.projects[0].path),
        "a selection is not an edit"
    );
    let _ = app.view_variant_manager_tab(id);

    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: r1.clone(),
            fitted: Some(false),
        },
    );
    assert_eq!(shown(&app, id).rows[0].fitted, [true, false]);
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );
    let _ = app.view_variant_manager_tab(id);

    // As on the sheets: the setting goes, the sheets' fit shows again.
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: r1,
            fitted: None,
        },
    );
    assert!(data(&app).variant_settings.is_empty());
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(shown(&app, id).rows[0].fitted, [true, false]);
    assert!(app.step_project_tab_history(id, true));
    assert!(data(&app).variant_settings.is_empty());
}

#[test]
fn base_cells_follow_the_sheets_and_take_no_edit() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let base = cell(None, "R1");
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: base.clone(),
            fitted: Some(false),
        },
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideEdited {
            parameter: VALUE_PARAMETER.into(),
            text: "2k".into(),
        },
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideSubmitted { cell: base.clone() },
    );
    assert!(data(&app).variant_settings.is_empty());
    assert!(!app.project_tab_history_available(id).0);
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(base));
    let _ = app.view_variant_manager_tab(id);
}

#[test]
fn a_typed_value_is_stored_on_enter_and_the_reset_brings_back_base() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = cell(Some("Lite"), "R1");
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(r1.clone()));
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideEdited {
            parameter: VALUE_PARAMETER.into(),
            text: "4k7".into(),
        },
    );
    assert!(
        data(&app).variant_settings.is_empty(),
        "typing stores nothing"
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideSubmitted { cell: r1.clone() },
    );
    let own = data(&app)
        .variant_settings
        .part(Some("Lite"), "R1")
        .expect("Lite's own R1");
    assert_eq!(own.parameters[VALUE_PARAMETER], "4k7");
    let matrix = shown(&app, id);
    assert!(matrix.rows[0].is_overridden_at(1));
    assert_eq!(matrix.changes(1), 2, "R1's value and R2's fit");
    assert_eq!(
        app.document_view.project_tabs[&id].variant_manager.draft,
        None
    );
    let _ = app.view_variant_manager_tab(id);

    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideReset {
            cell: r1,
            parameter: VALUE_PARAMETER.into(),
        },
    );
    assert!(data(&app).variant_settings.is_empty());
    assert!(app.step_project_tab_history(id, true));
    assert!(
        data(&app)
            .variant_settings
            .part(Some("Lite"), "R1")
            .is_some()
    );
}

#[test]
fn duplicate_copies_the_fits_the_sheets_give_and_opens_the_copy_name() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let task = manage(
        &mut app,
        id,
        VariantManagerMsg::DuplicateRequested {
            from: Some("Lite".into()),
        },
    );
    assert_eq!(data(&app).variant_definitions, ["Lite", "Lite copy"]);
    let copy = data(&app)
        .variant_settings
        .part(Some("Lite copy"), "R2")
        .expect("the sheet fit is carried");
    assert_eq!(copy.fitted, Some(false));
    let tab = &app.document_view.project_tabs[&id];
    assert_eq!(tab.variant_selected.as_deref(), Some("Lite copy"));
    assert_eq!(
        tab.variant_rename.as_ref().map(|draft| draft.from.as_str()),
        Some("Lite copy")
    );
    settle(&mut app, task);
    let matrix = shown(&app, id);
    assert_eq!(matrix.variants, ["Lite", "Lite copy"]);
    assert_eq!(matrix.rows[1].fitted, [true, false, false]);

    // One undo step takes the copy and its settings away.
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(data(&app).variant_definitions, ["Lite"]);
    assert!(data(&app).variant_settings.is_empty());
}

#[test]
fn duplicate_waits_for_a_parts_read_that_holds() {
    let (mut app, id, _dir) = app();
    app.open_project_tab(0);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::DuplicateRequested {
            from: Some("Lite".into()),
        },
    );
    assert_eq!(data(&app).variant_definitions, ["Lite"]);
}

#[test]
fn set_as_default_moves_the_star_and_undo_moves_it_back() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    assert_eq!(data(&app).default_variant(), None, "Base is the default");
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::DefaultPicked {
            name: Some("Lite".into()),
        },
    );
    assert_eq!(data(&app).default_variant(), Some("Lite"));
    assert_eq!(data(&app).active_variant, None, "the active variant stays");
    let _ = app.view_variant_manager_tab(id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::DefaultPicked { name: None },
    );
    assert_eq!(data(&app).default_variant(), None);
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(data(&app).default_variant(), Some("Lite"));
}

/// Settings has no surface yet: greyed, with the catalog's "not built yet"
/// tooltip. Compare variants and Open in BOM Studio are built (#2949).
#[test]
fn the_top_bar_actions_not_built_yet_read_from_the_catalog() {
    let i18n = signex_i18n::I18n::en_us();
    let labels: Vec<String> = super::bar::NOT_BUILT
        .iter()
        .map(|id| i18n.message_plain(*id, &[]))
        .collect();
    assert_eq!(labels, ["Settings..."]);
    assert_eq!(
        i18n.message_plain(signex_i18n::ids::VARIANT_MANAGER_NOT_BUILT, &[]),
        "Not built yet"
    );
}

#[test]
fn a_copy_takes_the_first_free_copy_name() {
    let (app, _id, _dir) = app();
    let mut project = data(&app).clone();
    let i18n = &app.ui_state.i18n;
    assert_eq!(
        super::cells::copy_name(&project, "Lite", i18n).as_deref(),
        Some("Lite copy")
    );
    project.variant_definitions.push("lite COPY".into());
    assert_eq!(
        super::cells::copy_name(&project, "Lite", i18n).as_deref(),
        Some("Lite copy 2")
    );
    assert_eq!(
        super::cells::copy_name(&project, "Base", i18n).as_deref(),
        Some("Base copy")
    );
}

/// Picking the fit the sheets already give a cell that sets none of its own
/// stores nothing, records no undo step and leaves the file clean.
#[test]
fn picking_the_fit_the_sheets_give_records_nothing() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: cell(Some("Lite"), "R2"),
            fitted: Some(false),
        },
    );
    assert!(data(&app).variant_settings.is_empty());
    assert!(!app.project_tab_history_available(id).0);
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );
}

/// A part given a value of its own in a variant and then left out there
/// shows Not fitted in its cell, not the value.
#[test]
fn a_not_fitted_cell_shows_no_value_of_its_own() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let r1 = cell(Some("Lite"), "R1");
    let _ = manage(&mut app, id, VariantManagerMsg::CellSelected(r1.clone()));
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideEdited {
            parameter: VALUE_PARAMETER.into(),
            text: "47k".into(),
        },
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::OverrideSubmitted { cell: r1.clone() },
    );
    assert!(shown(&app, id).rows[0].is_overridden_at(1));
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: r1,
            fitted: Some(false),
        },
    );
    let matrix = shown(&app, id);
    assert_eq!(matrix.rows[0].fitted, [true, false]);
    assert!(!matrix.rows[0].is_overridden_at(1), "Not fitted, no value");
    assert!(matrix.rows[0].differs_at(1));
    let _ = app.view_variant_manager_tab(id);
}

/// What is typed for one parameter is stored when the typing moves to
/// another parameter or another cell, as Enter stores it.
#[test]
fn typing_in_another_parameter_stores_the_first() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::CellSelected(cell(Some("Lite"), "R1")),
    );
    for (parameter, text) in [(VALUE_PARAMETER, "4k7"), (FOOTPRINT_PARAMETER, "R_0805")] {
        let _ = manage(
            &mut app,
            id,
            VariantManagerMsg::OverrideEdited {
                parameter: parameter.into(),
                text: text.into(),
            },
        );
    }
    let own = |app: &Signex| data(app).variant_settings.part(Some("Lite"), "R1").cloned();
    let first = own(&app).expect("the value is stored");
    assert_eq!(first.parameters[VALUE_PARAMETER], "4k7");
    assert!(
        !first.parameters.contains_key(FOOTPRINT_PARAMETER),
        "the footprint is still being typed"
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::CellSelected(cell(Some("Lite"), "R2")),
    );
    let both = own(&app).expect("R1's values");
    assert_eq!(both.parameters[FOOTPRINT_PARAMETER], "R_0805");
    assert_eq!(
        app.document_view.project_tabs[&id].variant_manager.draft,
        None
    );
}

/// A cell edit stores the placed part the parts read lists, which keeps the
/// setting on that part through a re-annotation.
#[test]
fn a_cell_edit_names_the_placed_part() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::FitPicked {
            cell: cell(Some("Lite"), "R1"),
            fitted: Some(false),
        },
    );
    let read = shown(&app, id).rows[0].link.clone();
    assert!(read.is_some(), "the read names R1's placed part");
    assert_eq!(data(&app).variant_settings.parts[0].part, read);
}
