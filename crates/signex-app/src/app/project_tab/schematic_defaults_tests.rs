//! #2026: a project default edited in the Project tab is a project edit (it
//! dirties the `.snxprj`, is saved in it and undoes), and the sheets of the
//! project draw and place with it.

use std::path::PathBuf;

use signex_types::project::SchematicDefaultField;

use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

/// A project with one open sheet, shown in the main window.
fn app() -> (Signex, PathBuf, iced::window::Id) {
    let dir = std::env::temp_dir().join(format!("signex-prj-def-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    let sheet = dir.join("root.snxsch");
    open_with(&mut app.document_state, &sheet, Default::default());
    app.open_project_tab(0);
    app.document_state.active_path = Some(sheet.clone());
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    (app, sheet, window)
}

fn edit(app: &mut Signex, field: SchematicDefaultField, text: &str) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SchematicDefaultEdited {
            project_id,
            field,
            text: text.to_string(),
        },
    )));
}

fn project_dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

#[test]
fn a_default_edit_dirties_saves_and_undoes() {
    let (mut app, _sheet, _window) = app();
    assert!(!project_dirty(&app));
    edit(&mut app, SchematicDefaultField::WireWidth, "0.3");
    assert!(project_dirty(&app));
    let bytes = signex_types::project::encode_project(&app.document_state.projects[0].data)
        .expect("encodes");
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["schematic_defaults"]["wire_width_mm"], 0.3);

    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .schematic_defaults
            .is_empty()
    );
    assert!(!project_dirty(&app), "undo returned to the saved state");
}

#[test]
fn the_sheet_draws_with_the_project_default() {
    let (mut app, _sheet, window) = app();
    assert_eq!(
        app.canvas_view_prefs(window)
            .schematic_defaults
            .wire_width_mm,
        0.254
    );
    edit(&mut app, SchematicDefaultField::WireWidth, "0.3");
    edit(&mut app, SchematicDefaultField::BusWidth, "0.9");
    let defaults = app.canvas_view_prefs(window).schematic_defaults;
    assert_eq!(defaults.wire_width_mm, 0.3);
    assert_eq!(defaults.bus_mm(), 0.9);
    assert_eq!(defaults.symbol_line_width_mm, 0.3, "pins follow the wire");
}

#[test]
fn placement_starts_from_the_project_defaults() {
    let (mut app, _sheet, _window) = app();
    edit(&mut app, SchematicDefaultField::TextSize, "12");
    edit(&mut app, SchematicDefaultField::ShapeStroke, "0.4");
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SchematicDefaultFontEdited {
            project_id,
            text: "Roboto".into(),
        },
    )));
    let pre = app.document_state.new_pre_placement(
        String::new(),
        crate::panels::PrePlacementKind::NetLabel,
        String::new(),
        (0.0, 0.0),
    );
    assert_eq!(pre.font_size_pt, 12);
    assert_eq!(pre.font, "Roboto");
    assert_eq!(
        app.document_state
            .active_schematic_defaults()
            .shape_stroke_mm(0.0),
        0.4
    );
}
