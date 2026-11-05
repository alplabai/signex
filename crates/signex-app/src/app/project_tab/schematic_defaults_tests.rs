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
    let a4 = signex_types::schematic::SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    open_with(&mut app.document_state, &sheet, a4);
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
    // Clicks are hit at the drawn width, on the canvas and in app handlers.
    assert_eq!(
        app.canvas_view_prefs(window).wire_widths().defaults,
        defaults
    );
    assert_eq!(app.active_wire_widths().defaults, defaults);
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

fn tab(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

#[test]
fn line_style_and_shape_fill_defaults_reach_drawing_and_placement() {
    use signex_types::net_class::NetLineStyle;
    use signex_types::schematic::FillType;
    let (mut app, _sheet, window) = app();
    let project_id = app.document_state.projects[0].id;
    tab(
        &mut app,
        ProjectTabMsg::SchematicDefaultLineStylePicked {
            project_id,
            line: Some(NetLineStyle::Dashed),
        },
    );
    tab(
        &mut app,
        ProjectTabMsg::SchematicDefaultShapeFillPicked {
            project_id,
            fill: Some(FillType::Background),
        },
    );
    assert!(project_dirty(&app));
    let defaults = app.canvas_view_prefs(window).schematic_defaults;
    assert_eq!(defaults.wire_line(None), NetLineStyle::Dashed);
    let pre = app.document_state.new_pre_placement(
        String::new(),
        crate::panels::PrePlacementKind::Other,
        String::new(),
        (0.0, 0.0),
    );
    assert_eq!(pre.shape_fill, FillType::Background);
    assert!(app.step_project_tab_history(project_id, true));
    assert!(app.step_project_tab_history(project_id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .schematic_defaults
            .is_empty()
    );
}

/// #2026: a label and a note placed on a project sheet store the project's
/// default font, as their size follows its default text size.
#[test]
fn placed_text_stores_the_project_font() {
    use crate::app::Tool;
    use crate::toolbar::ToolMessage;
    let (mut app, sheet, _window) = app();
    let project_id = app.document_state.projects[0].id;
    tab(
        &mut app,
        ProjectTabMsg::SchematicDefaultFontEdited {
            project_id,
            text: "Roboto".into(),
        },
    );
    for (tool, at) in [(Tool::Label, (20.32, 20.32)), (Tool::Text, (40.64, 40.64))] {
        let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
        let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
            world_x: at.0,
            world_y: at.1,
        }));
    }
    let doc = app.document_state.engines[&sheet].document();
    assert_eq!(doc.labels.len(), 1, "a label was placed");
    assert_eq!(doc.labels[0].font_family.as_deref(), Some("Roboto"));
    assert_eq!(doc.text_notes.len(), 1, "a note was placed");
    assert_eq!(doc.text_notes[0].font_family.as_deref(), Some("Roboto"));
}

/// #2034: the pre-placement X/Y readout shows Y from the page origin.
#[test]
fn the_pre_placement_readout_reads_y_from_the_page_origin() {
    use signex_types::schematic::PageOrigin;
    let (mut app, sheet, _window) = app();
    let readout = |app: &Signex| {
        let pre = app.document_state.new_pre_placement(
            String::new(),
            crate::panels::PrePlacementKind::Other,
            String::new(),
            (7.0, 8.0),
        );
        (pre.cursor_x_mm, pre.cursor_y_mm)
    };
    assert_eq!(readout(&app), (7.0, 210.0 - 8.0), "Lower Left, A4");
    let engine = app.document_state.engines.get_mut(&sheet).expect("engine");
    engine
        .execute(signex_engine::Command::SetPageOrigin {
            origin: PageOrigin::UpperLeft,
        })
        .expect("origin");
    assert_eq!(readout(&app), (7.0, 8.0));
}
