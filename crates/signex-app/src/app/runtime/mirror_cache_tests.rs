//! The panel mirrors `finish_update` keeps are rebuilt only when what they
//! read changed: a resize or a hover rebuilds none of them, while an edit
//! still reaches each one.

use std::path::PathBuf;

use signex_sketch::id::ConstraintId;
use signex_types::schematic::{HarnessConnector, Point, SchUnion, SelectedItem, SelectedKind};

use super::super::*;
use super::sketch_canvas_text::CONFLICT_COUNTS;
use crate::app::handlers::dock::HARNESS_FACT_BUILDS;
use crate::app::unions::UNIONS_VIEW_BUILDS;
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_with_selected_wire;
use crate::panels::PanelKind;

const FOOTPRINT: &str = "words.snxfpt";

/// How often this thread counted conflicts, built the harness facts and
/// built the Unions view.
fn builds() -> (usize, usize, usize) {
    (
        CONFLICT_COUNTS.with(std::cell::Cell::get),
        HARNESS_FACT_BUILDS.with(std::cell::Cell::get),
        UNIONS_VIEW_BUILDS.with(std::cell::Cell::get),
    )
}

/// A sheet with a selected wire, the Unions panel open and a footprint
/// editor open, every mirror built once.
fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_with_selected_wire();
    app.document_view.dock.show_panel(PanelKind::Unions);
    let path = PathBuf::from(FOOTPRINT);
    app.document_state.footprint_editors.insert(
        path.clone(),
        FootprintDocument::new(
            path,
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty(
                "words",
            )),
        ),
    );
    app.sync_panel_ctx_mirrors();
    app.update_selection_info();
    (app, main)
}

/// A window resize, an idle pointer move and a canvas hover.
fn resize_and_hover(app: &mut Signex, main: iced::window::Id) {
    let _ = app.update(Message::Window(WindowMsg::WindowResizedFor(
        main, 1600.0, 1000.0,
    )));
    let _ = app.update(Message::Ui(UiMsg::DragMove(300.0, 200.0)));
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CursorAt {
        x: 30.0,
        y: 20.0,
        zoom_pct: 100.0,
    }));
}

#[test]
fn a_resize_and_a_hover_rebuild_no_panel_mirror() {
    let (mut app, main) = app();
    let before = builds();
    resize_and_hover(&mut app, main);
    app.update_selection_info();
    assert_eq!(builds(), before, "(conflict counts, harness, unions)");
}

#[test]
fn a_rejected_edit_still_reaches_the_conflict_badge() {
    let (mut app, main) = app();
    let counted = builds().0;
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(&PathBuf::from(FOOTPRINT))
        .expect("editor");
    editor
        .state
        .conflict
        .record_rejection(vec![ConstraintId::new()], vec![], vec![]);
    resize_and_hover(&mut app, main);
    let text = &app.document_state.footprint_editors[&PathBuf::from(FOOTPRINT)]
        .state
        .canvas_text;
    assert_eq!(text.conflict_count, 1);
    assert!(!text.conflict_badge.is_empty());
    assert_eq!(builds().0, counted + 1, "counted once for the one change");
}

#[test]
fn a_placed_harness_connector_reaches_the_harness_panel() {
    let (mut app, _) = app();
    let connector = HarnessConnector {
        uuid: uuid::Uuid::now_v7(),
        harness_type: "CAM".into(),
        position: Point::new(80.0, 40.0),
        width: 10.0,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: vec![],
        locked: false,
    };
    let id = connector.uuid;
    assert!(app.apply_engine_command(
        signex_engine::Command::PlaceHarnessConnector { connector },
        false,
        true,
    ));
    app.replace_selection(vec![SelectedItem::new(id, SelectedKind::HarnessConnector)]);
    let shown = app.document_view.panel_ctx.harness.connector.as_ref();
    assert_eq!(shown.map(|c| c.uuid), Some(id));
}

#[test]
fn the_unions_panel_follows_a_sheet_change() {
    let (mut app, _) = app();
    let wire = app.interaction_state.active_canvas().selected[0].uuid;
    let rows = |app: &Signex| -> Vec<String> {
        let view = &app.document_view.panel_ctx.unions;
        view.groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| r.name.clone()))
            .collect()
    };
    assert!(rows(&app).is_empty());
    assert!(app.apply_engine_command(
        signex_engine::Command::CreateUnion {
            union: SchUnion {
                uuid: uuid::Uuid::now_v7(),
                name: "Camera".into(),
                tags: vec![],
                members: vec![wire],
                locked: false,
            },
        },
        false,
        true,
    ));
    app.sync_panel_ctx_mirrors();
    assert_eq!(rows(&app), ["Camera"]);
}
