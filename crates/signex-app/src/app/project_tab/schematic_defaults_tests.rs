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
    // A size placement holds in whole points: every path rounds it alike.
    edit(&mut app, SchematicDefaultField::TextSize, "10.4");
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
    let pp_mm = f64::from(
        app.document_state
            .new_pre_placement(
                String::new(),
                crate::panels::PrePlacementKind::NetLabel,
                String::new(),
                (0.0, 0.0),
            )
            .font_size_pt,
    ) * signex_types::schematic::SCHEMATIC_PT_TO_MM;
    assert_eq!(doc.labels[0].font_size, pp_mm, "label size = form size");
    assert_eq!(doc.text_notes[0].font_size, pp_mm, "note size = form size");
}

/// #2034: the pre-placement X/Y readout shows Y from the page origin, and
/// follows an origin change while the tool is armed.
#[test]
fn the_pre_placement_readout_reads_y_from_the_page_origin() {
    use crate::panels::PageOrigin;
    let (mut app, _sheet, _window) = app();
    app.document_state.panel_ctx.pre_placement = Some(app.document_state.new_pre_placement(
        String::new(),
        crate::panels::PrePlacementKind::Other,
        String::new(),
        (7.0, 8.0),
    ));
    let readout = |app: &Signex| {
        let pre = app
            .document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .expect("armed");
        crate::panels::pre_placement_readout(pre, &app.document_state.panel_ctx)
    };
    app.refresh_panel_ctx();
    assert_eq!(readout(&app), "7.00, 202.00", "Lower Left, A4");
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::SetPageOrigin(PageOrigin::UpperLeft),
    )));
    assert_eq!(readout(&app), "7.00, 8.00");
}

/// Review of #2026: every defaults change (an edit, a pick, an undo) redraws
/// every schematic window, the undocked ones too.
#[test]
fn a_defaults_change_clears_every_schematic_canvas() {
    let (mut app, _sheet, _window) = app();
    let undocked = iced::window::Id::unique();
    app.interaction_state
        .canvases
        .insert(undocked, crate::canvas::CanvasSlot::new());
    let clears = |app: &Signex| {
        (
            app.interaction_state.canvas.content_cache_clears,
            app.interaction_state.canvases[&undocked].content_cache_clears,
        )
    };
    let project_id = app.document_state.projects[0].id;
    let check = |app: &mut Signex, change: &dyn Fn(&mut Signex)| {
        let before = clears(app);
        change(app);
        let after = clears(app);
        assert!(
            after.0 > before.0 && after.1 > before.1,
            "{before:?} -> {after:?}"
        );
    };
    check(&mut app, &|app| {
        edit(app, SchematicDefaultField::WireWidth, "1.0")
    });
    check(&mut app, &|app| {
        tab(
            app,
            ProjectTabMsg::SchematicDefaultLineStylePicked {
                project_id,
                line: Some(signex_types::net_class::NetLineStyle::Dotted),
            },
        )
    });
    check(&mut app, &|app| {
        assert!(app.step_project_tab_history(project_id, true));
    });
}

/// Review of #2026: a typed default outside its range is stored clamped,
/// the field says so, and a file value out of range resolves clamped
/// without being rewritten.
#[test]
fn an_out_of_range_default_is_clamped_and_reported() {
    use signex_types::project::MAX_DEFAULT_WIDTH_MM;
    let (mut app, _sheet, _window) = app();
    edit(&mut app, SchematicDefaultField::WireWidth, "12");
    let stored = app.document_state.projects[0]
        .data
        .schematic_defaults
        .wire_width_mm;
    assert_eq!(stored, Some(MAX_DEFAULT_WIDTH_MM));
    let draft = "12".to_string();
    let note =
        super::schematic_defaults::clamp_note(SchematicDefaultField::WireWidth, Some(&draft));
    assert!(
        note.is_some_and(|n| n.contains("5")),
        "the field reports the clamp"
    );
    edit(&mut app, SchematicDefaultField::WireWidth, "0.01");
    assert_eq!(
        app.document_state.projects[0]
            .data
            .schematic_defaults
            .wire_width_mm,
        Some(0.15)
    );
    // Text: a tiny size never places at 0 pt.
    edit(&mut app, SchematicDefaultField::TextSize, "0.2");
    let pre = app.document_state.new_pre_placement(
        String::new(),
        crate::panels::PrePlacementKind::Other,
        String::new(),
        (0.0, 0.0),
    );
    assert_eq!(pre.font_size_pt, 1);

    let raw = signex_types::project::SchematicDefaultOverrides {
        bus_width_mm: Some(50.0),
        ..Default::default()
    };
    assert_eq!(
        raw.resolve().bus_width_mm,
        10.0,
        "a file value resolves clamped"
    );
    assert_eq!(raw.bus_width_mm, Some(50.0), "and is not rewritten");
}

/// Review of #2026: text that is not a usable number keeps the stored
/// default and the draft (`numeric_field`'s rule); only empty text clears.
/// A half-typed number joins the same undo step as the full one.
#[test]
fn unusable_input_keeps_the_default_and_only_empty_text_clears() {
    let (mut app, _sheet, _window) = app();
    let wire = |app: &Signex| {
        app.document_state.projects[0]
            .data
            .schematic_defaults
            .wire_width_mm
    };
    edit(&mut app, SchematicDefaultField::WireWidth, "0.");
    assert_eq!(wire(&app), None, "mid-edit changes nothing");
    edit(&mut app, SchematicDefaultField::WireWidth, "0.3");
    for text in ["0,4", "x", "-1", "0"] {
        edit(&mut app, SchematicDefaultField::WireWidth, text);
        assert_eq!(wire(&app), Some(0.3), "{text:?} keeps the default");
    }
    let project_id = app.document_state.projects[0].id;
    let draft = app.document_state.project_tabs[&project_id]
        .default_drafts
        .get(&SchematicDefaultField::WireWidth)
        .cloned();
    assert_eq!(draft.as_deref(), Some("0"), "the box keeps the typed text");
    assert!(app.step_project_tab_history(project_id, true));
    assert_eq!(wire(&app), None, "0. and 0.3 were one step");
    assert!(app.step_project_tab_history(project_id, false));
    assert_eq!(wire(&app), Some(0.3));
    edit(&mut app, SchematicDefaultField::WireWidth, "");
    assert_eq!(wire(&app), None, "empty text clears");
}

/// Review of #2026: a junction default below its draw floor is clamped to
/// 0.7 mm and the field says so.
#[test]
fn a_junction_default_below_the_floor_is_clamped_and_noted() {
    let draft = "0.3".to_string();
    let note = super::schematic_defaults::clamp_note(
        SchematicDefaultField::JunctionDiameter,
        Some(&draft),
    );
    assert!(note.is_some_and(|n| n.contains("0.7")), "noted");
    let (mut app, _sheet, _window) = app();
    edit(&mut app, SchematicDefaultField::JunctionDiameter, "0.3");
    assert_eq!(
        app.document_state.projects[0]
            .data
            .schematic_defaults
            .junction_diameter_mm,
        Some(0.7)
    );
}

/// Review of #2026: a late edit for a project that is no longer loaded
/// creates no tab state for it.
#[test]
fn a_late_edit_for_a_closed_project_leaves_no_tab_state() {
    let (mut app, _sheet, _window) = app();
    let gone = app.document_state.mint_project_id();
    tab(
        &mut app,
        ProjectTabMsg::SchematicDefaultEdited {
            project_id: gone,
            field: SchematicDefaultField::WireWidth,
            text: "0.3".into(),
        },
    );
    assert!(!app.document_state.project_tabs.contains_key(&gone));
}

/// Review of #2026: a shape placed on a project sheet with Width untouched
/// stores the project's shape stroke default.
#[test]
fn a_rectangle_placed_on_a_project_sheet_stores_the_project_shape_stroke() {
    use crate::app::Tool;
    use crate::toolbar::ToolMessage;
    use signex_types::schematic::SchDrawing;
    let (mut app, sheet, _window) = app();
    edit(&mut app, SchematicDefaultField::ShapeStroke, "0.4");
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));
    for at in [(2.54, 2.54), (12.7, 7.62)] {
        let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
            world_x: at.0,
            world_y: at.1,
        }));
    }
    let doc = app.document_state.engines[&sheet].document();
    let [SchDrawing::Rect { width, .. }] = doc.drawings.as_slice() else {
        panic!("expected one rectangle, got {:?}", doc.drawings);
    };
    assert_eq!(*width, 0.4);
}
