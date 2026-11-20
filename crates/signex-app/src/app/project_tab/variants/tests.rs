//! #1293: the Variants section in the Variant Manager layout, driven through
//! `Signex::update`. `+` adds "Variant N" and opens its name in place, the
//! list selection is not the active variant, the parts matrix is read off the
//! UI thread from the project's sheets, and Only rows that differ hides the
//! parts every variant fits as Base does.

use std::sync::Arc;

use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::super::VariantRenameDraft;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};

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

/// A project whose one sheet is open with R1 (fitted everywhere), R2 (not
/// fitted in Lite) and a power port; Lite is defined. The folder lives as
/// long as the returned guard.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let mut power = part("#PWR01", "GND", &[]);
    power.is_power = true;
    let sheet = SchematicSheet {
        symbols: vec![
            part("R2", "10k", &[("Fitted@Lite", "no")]),
            part("R1", "1k", &[]),
            power,
        ],
        ..Default::default()
    };
    open_with(&mut app.document_state, &dir.path().join("a.snxsch"), sheet);
    app.document_state.projects[0].data.variant_definitions = vec!["Lite".to_string()];
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(msg)))
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

fn show_variants(app: &mut Signex, id: ProjectId) {
    let task = send(
        app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Variants,
        },
    );
    settle(app, task);
}

fn tab(app: &Signex, id: ProjectId) -> &super::super::ProjectTabState {
    &app.document_state.project_tabs[&id]
}

fn matrix(app: &Signex, id: ProjectId) -> &signex_output::VariantMatrix {
    tab(app, id)
        .variant_parts
        .as_deref()
        .expect("the parts were read")
        .as_ref()
        .expect("the parts were read without a problem")
}

fn definitions(app: &Signex) -> Vec<String> {
    app.document_state.projects[0]
        .data
        .variant_definitions
        .clone()
}

#[test]
fn showing_the_section_reads_the_parts_matrix_off_the_ui_thread() {
    let (mut app, id, _dir) = app();
    show_variants(&mut app, id);
    assert!(!tab(&app, id).variant_parts_reading);
    let read = matrix(&app, id);
    assert_eq!(read.variants, ["Lite"]);
    let rows: Vec<(&str, &str, &[bool])> = read
        .rows
        .iter()
        .map(|row| {
            (
                row.designator.as_str(),
                row.value.as_str(),
                row.fitted.as_slice(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("R1", "1k", &[true, true][..]),
            ("R2", "10k", &[true, false][..]),
        ],
        "one row per part in designator order, power ports left out"
    );
    assert_eq!(read.differing_cells(), 1);
    let _ = app.view_project_tab(id);
}

/// Maintainer decision 2026-11-20 (Caner): the Value column shows the value
/// the BOM lists, an `=` value evaluated on its sheet with the project's
/// parameters, and the parts filter matches that value.
#[test]
fn the_value_column_shows_an_expression_value_evaluated_as_the_bom_lists_it() {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let sheet = SchematicSheet {
        symbols: vec![
            part("R1", "= VCC / 10mA", &[]),
            part("R2", "= Rbase * 2", &[("Rbase", "10kΩ")]),
        ],
        ..Default::default()
    };
    open_with(&mut app.document_state, &dir.path().join("a.snxsch"), sheet);
    app.document_state.projects[0]
        .data
        .parameters
        .insert("VCC".to_string(), "5V".to_string());
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    show_variants(&mut app, id);
    let values: Vec<(&str, &str)> = matrix(&app, id)
        .rows
        .iter()
        .map(|row| (row.designator.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(values, [("R1", "500Ω"), ("R2", "20kΩ")]);
    let shown = super::matrix::shown_rows(matrix(&app, id), "20k", false);
    let names: Vec<&str> = shown.iter().map(|row| row.designator.as_str()).collect();
    assert_eq!(names, ["R2"]);
    let _ = app.view_project_tab(id);
}

#[test]
fn only_rows_that_differ_hides_the_parts_every_variant_fits_as_base_does() {
    let (mut app, id, _dir) = app();
    show_variants(&mut app, id);
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantDifferingToggled { project_id: id },
    );
    assert!(tab(&app, id).variants_only_differ);
    let shown = super::matrix::shown_rows(matrix(&app, id), "", true);
    let names: Vec<&str> = shown.iter().map(|row| row.designator.as_str()).collect();
    assert_eq!(names, ["R2"]);
    let _ = app.view_project_tab(id);

    let _ = send(
        &mut app,
        ProjectTabMsg::VariantFilterEdited {
            project_id: id,
            text: "1k".into(),
        },
    );
    assert_eq!(tab(&app, id).variant_filter, "1k");
    assert!(super::matrix::shown_rows(matrix(&app, id), "1k", true).is_empty());
    let all = super::matrix::shown_rows(matrix(&app, id), "R*", false);
    assert_eq!(all.len(), 2);
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantDifferingToggled { project_id: id },
    );
    assert!(!tab(&app, id).variants_only_differ);
}

#[test]
fn plus_adds_the_first_free_variant_name_and_opens_it_for_typing() {
    let (mut app, id, _dir) = app();
    let _ = send(
        &mut app,
        ProjectTabMsg::NewVariantRequested { project_id: id },
    );
    assert_eq!(definitions(&app), ["Lite", "Variant 1"]);
    let state = tab(&app, id);
    assert_eq!(state.variant_selected.as_deref(), Some("Variant 1"));
    assert_eq!(
        state.variant_rename,
        Some(VariantRenameDraft {
            from: "Variant 1".into(),
            text: "Variant 1".into(),
        })
    );
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );

    // Typing a name and Enter renames it; the selection follows.
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantRenameEdited {
            project_id: id,
            from: "Variant 1".into(),
            text: "Pro".into(),
        },
    );
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantRenameSubmitted { project_id: id },
    );
    assert_eq!(definitions(&app), ["Lite", "Pro"]);
    assert_eq!(tab(&app, id).variant_selected.as_deref(), Some("Pro"));
    assert_eq!(tab(&app, id).variant_rename, None);

    // The next `+` takes Variant 1 again: the name is free.
    let _ = send(
        &mut app,
        ProjectTabMsg::NewVariantRequested { project_id: id },
    );
    assert_eq!(definitions(&app), ["Lite", "Pro", "Variant 1"]);
    // Enter on an unchanged name keeps it.
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantRenameSubmitted { project_id: id },
    );
    assert_eq!(definitions(&app), ["Lite", "Pro", "Variant 1"]);

    // Three steps back: the add, the rename, the first add.
    for _ in 0..3 {
        assert!(app.step_project_tab_history(id, true));
    }
    assert_eq!(definitions(&app), ["Lite"]);
}

#[test]
fn the_first_free_name_skips_taken_ones_ignoring_case() {
    let (app, _id, _dir) = app();
    let mut data = app.document_state.projects[0].data.clone();
    data.variant_definitions = vec!["Variant 1".into(), "variant 2".into()];
    assert_eq!(
        super::edits::first_free_variant_name(&data, &app.ui_state.i18n).as_deref(),
        Some("Variant 3")
    );
}

#[test]
fn selecting_a_variant_does_not_make_it_active() {
    let (mut app, id, _dir) = app();
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantSelected {
            project_id: id,
            name: Some("Lite".into()),
        },
    );
    assert_eq!(tab(&app, id).variant_selected.as_deref(), Some("Lite"));
    assert_eq!(app.document_state.projects[0].data.active_variant, None);
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.projects[0].path),
        "a selection is not an edit"
    );

    // Base is selected by its name; a draft of another variant is dropped.
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantRenameStarted {
            project_id: id,
            name: "Lite".into(),
        },
    );
    assert!(tab(&app, id).variant_rename.is_some());
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantSelected {
            project_id: id,
            name: None,
        },
    );
    assert_eq!(tab(&app, id).variant_selected.as_deref(), Some("Base"));
    assert_eq!(tab(&app, id).variant_rename, None);

    // Deleting the selected variant hands the selection back to the active one.
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantSelected {
            project_id: id,
            name: Some("Lite".into()),
        },
    );
    let _ = send(
        &mut app,
        ProjectTabMsg::VariantRemoved {
            project_id: id,
            name: "Lite".into(),
        },
    );
    assert!(definitions(&app).is_empty());
    assert_eq!(tab(&app, id).variant_selected, None);
}

#[test]
fn a_project_with_base_only_shows_the_empty_state_and_its_button_adds_a_variant() {
    let (mut app, id, _dir) = app();
    app.document_state.projects[0]
        .data
        .variant_definitions
        .clear();
    show_variants(&mut app, id);
    assert_eq!(matrix(&app, id).variants.len(), 0);
    assert_eq!(matrix(&app, id).rows[0].fitted, [true]);
    let _ = app.view_project_tab(id);
    let _ = send(
        &mut app,
        ProjectTabMsg::NewVariantRequested { project_id: id },
    );
    assert_eq!(definitions(&app), ["Variant 1"]);
    let _ = app.view_project_tab(id);
}

#[test]
fn a_sheet_edit_makes_the_read_stale_and_an_older_read_is_dropped() {
    let (mut app, id, _dir) = app();
    show_variants(&mut app, id);
    let first = tab(&app, id).variant_parts_generation;
    // Nothing changed: no new read starts.
    let _ = app.read_variant_parts(id, false);
    assert_eq!(tab(&app, id).variant_parts_generation, first);

    app.document_state.sheet_edit_epoch += 1;
    let _ = app.read_variant_parts(id, false);
    assert_eq!(tab(&app, id).variant_parts_generation, first + 1);
    assert!(tab(&app, id).variant_parts_reading);

    // The first read's result lands late: it is dropped.
    let late = Arc::new(Err(super::PartsReadError::NoRootSheet));
    let epoch = app.document_state.sheet_edit_epoch;
    app.install_variant_parts(id, first, epoch, late);
    assert!(
        tab(&app, id)
            .variant_parts
            .as_deref()
            .is_some_and(Result::is_ok)
    );
    assert!(tab(&app, id).variant_parts_reading);
}

/// A failed parts read shows a catalog message per cause; only the path is
/// user content.
#[test]
fn a_failed_read_names_its_cause_from_the_catalog() {
    use signex_i18n::{I18n, ids, t};
    let i18n = I18n::en_us();
    let no_root = super::read_failed(&i18n, &super::PartsReadError::NoRootSheet).to_string();
    assert_eq!(
        no_root,
        t!(i18n, ids::PROJECT_VARIANTS_READ_NO_ROOT).to_string()
    );
    let unreadable = super::read_failed(
        &i18n,
        &super::PartsReadError::RootUnreadable(std::path::PathBuf::from("top.snxsch")),
    )
    .to_string();
    assert!(unreadable.contains("top.snxsch"), "{unreadable}");
    assert_ne!(unreadable, no_root);
}
