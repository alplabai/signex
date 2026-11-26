//! #2949, the GUI path of the Variant Manager's top bar actions: Compare
//! variants shows two variants side by side in place of the cards and the
//! matrix (mockup row 59, view d), the sides are picked and swapped, Close
//! brings the matrix back, and Open in BOM Studio opens the BOM preview
//! with the variant picked. Neither action changes the project.

use super::manager::{CompareSides, VariantManagerMsg};
use super::manager_tests::{app, data, manage, open_manager};
use crate::app::Signex;
use crate::app::state::ProjectId;

fn sides(app: &Signex, id: ProjectId) -> Option<CompareSides> {
    app.document_view.project_tabs[&id]
        .variant_manager
        .compare
        .clone()
}

fn side(name: Option<&str>) -> Option<String> {
    name.map(str::to_string)
}

#[test]
fn compare_opens_on_base_against_the_first_variant_and_closes() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let before = serde_json::to_value(data(&app)).expect("the project serialises");
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    assert_eq!(
        sides(&app, id),
        Some(CompareSides {
            from: None,
            to: side(Some("Lite")),
        })
    );
    let _ = app.view_variant_manager_tab(id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareClosed);
    assert_eq!(sides(&app, id), None);
    let _ = app.view_variant_manager_tab(id);
    assert_eq!(
        serde_json::to_value(data(&app)).expect("the project serialises"),
        before,
        "comparing changes nothing"
    );
    assert!(!app.project_tab_history_available(id).0);
}

/// The right side starts on the variant selected in the list.
#[test]
fn compare_opens_on_the_selected_variant() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Pro".into());
    app.document_view
        .project_tabs
        .get_mut(&id)
        .expect("the tab")
        .variant_selected = Some("pro".into());
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    assert_eq!(sides(&app, id).and_then(|s| s.to), side(Some("Pro")));
}

#[test]
fn compare_does_not_open_without_a_variant_besides_base() {
    let (mut app, id, _dir) = app();
    app.document_state.projects[0]
        .data
        .variant_definitions
        .clear();
    open_manager(&mut app, id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    assert_eq!(sides(&app, id), None);
}

#[test]
fn a_side_is_picked_and_the_sides_swap() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::CompareSidePicked {
            from: true,
            name: side(Some("Lite")),
        },
    );
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::CompareSidePicked {
            from: false,
            name: side(Some("base")),
        },
    );
    assert_eq!(
        sides(&app, id),
        Some(CompareSides {
            from: side(Some("Lite")),
            to: None,
        }),
        "Base's name picks Base"
    );
    let _ = manage(&mut app, id, VariantManagerMsg::CompareSwapped);
    assert_eq!(
        sides(&app, id),
        Some(CompareSides {
            from: None,
            to: side(Some("Lite")),
        })
    );
    let _ = app.view_variant_manager_tab(id);
}

/// A pick while the comparison is closed opens nothing.
#[test]
fn a_pick_or_swap_while_closed_changes_nothing() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareSwapped);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::CompareSidePicked {
            from: true,
            name: side(Some("Lite")),
        },
    );
    assert_eq!(sides(&app, id), None);
}

/// The tab shows what `VariantMatrix::compare` gives over the read: R2,
/// left out of Lite on its sheet, is the one difference.
#[test]
fn the_comparison_shown_lists_the_parts_that_differ() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    let read = app.document_view.project_tabs[&id]
        .variant_parts
        .as_deref()
        .expect("the parts were read")
        .as_ref()
        .expect("the parts were read without a problem");
    let compared = read
        .compare(&data(&app).variant_settings, None, Some("Lite"))
        .expect("both sides are read");
    let designators: Vec<&str> = compared
        .parts
        .iter()
        .map(|part| part.designator.as_str())
        .collect();
    assert_eq!(designators, ["R2"]);
    assert_eq!((compared.from_fitted, compared.to_fitted), (2, 1));
    let _ = app.view_variant_manager_tab(id);
}

/// A side the project no longer defines shows the reason, not a panic.
#[test]
fn an_unknown_side_shows_why_it_cannot_compare() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(&mut app, id, VariantManagerMsg::CompareOpened);
    app.document_view
        .project_tabs
        .get_mut(&id)
        .and_then(|tab| tab.variant_manager.compare.as_mut())
        .expect("the comparison is open")
        .to = side(Some("Gone"));
    let _ = app.view_variant_manager_tab(id);
}

#[test]
fn open_in_bom_studio_opens_the_bom_with_the_variant_picked() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::BomRequested {
            variant: side(Some("Lite")),
        },
    );
    let preview = app
        .document_view
        .bom_preview
        .as_ref()
        .expect("the BOM preview opened");
    assert_eq!(preview.options.active_variant.as_deref(), Some("Lite"));
    assert_eq!(
        preview.source,
        app.document_state.projects[0].dir().join("a.snxsch")
    );

    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::BomRequested { variant: None },
    );
    let preview = app.document_view.bom_preview.as_ref().expect("reopened");
    assert_eq!(preview.options.active_variant, None, "None is Base");
}

#[test]
fn open_in_bom_studio_without_an_open_sheet_opens_nothing() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    app.document_state.engines.clear();
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::BomRequested {
            variant: side(Some("Lite")),
        },
    );
    assert!(app.document_view.bom_preview.is_none());
}

/// The built top bar actions read from the catalog, and Open in BOM Studio
/// carries the variant selected in the list.
#[test]
fn the_built_top_bar_actions_send_their_messages() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    app.document_view
        .project_tabs
        .get_mut(&id)
        .expect("the tab")
        .variant_selected = Some("Lite".into());
    let project = app.document_state.project_by_id(id).expect("the project");
    let tab = app.document_view.project_tabs.get(&id);
    let i18n = app.ui_state.i18n.as_ref();
    let m = super::Manager::new(
        project,
        tab,
        app.document_state.sheet_edit_epoch,
        i18n,
        &app.document_view.panel_ctx.tokens,
    );
    let actions = super::bar::actions(&m);
    let labels: Vec<String> = actions
        .iter()
        .map(|(label, _)| i18n.message_plain(*label, &[]))
        .collect();
    assert_eq!(labels, ["Compare variants...", "Open in BOM Studio"]);
    assert!(matches!(actions[0].1, VariantManagerMsg::CompareOpened));
    assert!(matches!(
        &actions[1].1,
        VariantManagerMsg::BomRequested { variant } if variant.as_deref() == Some("Lite")
    ));
}
