//! #1079 — the canvas draws the sheet frame, zone markers and title block,
//! lowered by the same function the PDF uses and filled from the sheet's
//! title block, and it follows the sheet's paper size.

use signex_app::app::{Message, Signex, TabInfo, TabKind, WindowMsg};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, PaperEdge};
use signex_output::template::FramePrimitive;
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::PathBuf;

fn app_with_title_block() -> Signex {
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        title_block: HashMap::from([
            ("title".to_string(), "Power supply".to_string()),
            ("rev".to_string(), "B".to_string()),
        ]),
        ..SchematicSheet::default()
    };
    let path = PathBuf::from("power.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "power".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn set_paper(app: &mut Signex, size: &str) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SetPaperSize(
        size.to_string(),
    ))));
}

fn frame(app: &Signex) -> &[FramePrimitive] {
    &app.interaction_state.active_canvas().sheet_frame
}

fn text_at(app: &Signex, wanted: &str) -> Option<(f64, f64)> {
    frame(app).iter().find_map(|p| match p {
        FramePrimitive::Text { x, y, text, .. } if text == wanted => Some((*x, *y)),
        _ => None,
    })
}

#[test]
fn the_canvas_shows_the_title_block_values_and_zone_markers() {
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");

    assert!(text_at(&app, "Power supply").is_some(), "{:?}", frame(&app));
    assert!(text_at(&app, "Rev").is_some() && text_at(&app, "B").is_some());
    assert!(text_at(&app, "1 of 1").is_some());
    assert!(text_at(&app, "power.snxsch").is_some());
    for zone in ["1", "8", "A", "F"] {
        assert!(text_at(&app, zone).is_some(), "zone {zone}");
    }
}

#[test]
fn the_title_block_follows_the_paper_size() {
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");
    let (a4_x, a4_y) = text_at(&app, "Power supply").unwrap();
    // A4 landscape is 297 x 210 mm; the title's value sits under its caption
    // (0.7 mm in, 2.6 mm down) in the top cell of a 110 x 28 mm block in the
    // bottom-right of the 10 mm-margin frame (#1201).
    assert!((a4_x - 177.7).abs() < 1e-6 && (a4_y - 174.6).abs() < 1e-6);

    set_paper(&mut app, "A3");
    let (a3_x, a3_y) = text_at(&app, "Power supply").unwrap();
    assert!((a3_x - 300.7).abs() < 1e-6 && (a3_y - 261.6).abs() < 1e-6);
}

#[test]
fn an_undocked_tab_window_opens_with_the_sheet_frame() {
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");
    let window_id = iced::window::Id::unique();
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: PathBuf::from("power.snxsch"),
        id: window_id,
    }));
    let canvas = app
        .interaction_state
        .canvases
        .get(&window_id)
        .expect("per-window canvas");
    assert!(
        canvas.sheet_frame.iter().any(|p| matches!(
            p,
            FramePrimitive::Text { text, .. } if text == "Power supply"
        )),
        "{:?}",
        canvas.sheet_frame
    );
}

/// Loose sheets are numbered as the PDF export numbers them: the active
/// one first, out of every open loose sheet.
#[test]
fn loose_sheets_are_numbered_as_the_export_numbers_them() {
    let mut app = app_with_title_block();
    let second = PathBuf::from("second.snxsch");
    app.document_state.engines.insert(
        second.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "second".to_string(),
        path: second,
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    set_paper(&mut app, "A4");
    assert!(text_at(&app, "1 of 2").is_some(), "{:?}", frame(&app));
}

/// #1082 — the canvas reads the typed sheet-size model: ANSI and portrait
/// tokens size the page.
#[test]
fn the_canvas_sizes_the_page_from_the_typed_sheet_size() {
    let mut app = app_with_title_block();

    set_paper(&mut app, "ANSI D");
    let canvas = app.interaction_state.active_canvas();
    assert_eq!(
        (canvas.paper_width_mm, canvas.paper_height_mm),
        (863.6, 558.8)
    );

    set_paper(&mut app, "A4 portrait");
    let canvas = app.interaction_state.active_canvas();
    assert_eq!(
        (canvas.paper_width_mm, canvas.paper_height_mm),
        (210.0, 297.0)
    );
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn stored_paper(app: &Signex) -> String {
    let path = app.document_state.active_path.clone().expect("active path");
    app.document_state.engines[&path]
        .document()
        .paper_size
        .clone()
}

fn page_mm(app: &Signex) -> (f32, f32) {
    let canvas = app.interaction_state.active_canvas();
    (canvas.paper_width_mm, canvas.paper_height_mm)
}

/// #1082 — the orientation toggle turns the sheet through the engine: the
/// stored token, the canvas and undo all follow, and a later size pick keeps
/// the orientation.
#[test]
fn the_orientation_toggle_is_an_undoable_engine_edit() {
    use signex_types::schematic::SheetOrientation;
    let mut app = app_with_title_block();

    panel(
        &mut app,
        PanelMsg::SetSheetOrientation(SheetOrientation::Portrait),
    );
    assert_eq!(stored_paper(&app), "A4 portrait");
    assert_eq!(page_mm(&app), (210.0, 297.0));

    panel(&mut app, PanelMsg::SetPaperSize("A3".to_string()));
    assert_eq!(stored_paper(&app), "A3 portrait");

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(stored_paper(&app), "A4 portrait");
    assert_eq!(page_mm(&app), (210.0, 297.0));
}

/// #1082 — Custom width and height persist in the document and never leave
/// the range the PDF page-setup modal accepts.
#[test]
fn custom_edges_persist_and_stay_within_the_pdf_limits() {
    use signex_types::schematic::{SheetOrientation, SheetSize};
    let mut app = app_with_title_block();

    type_edge(&mut app, PaperEdge::Width, "400");
    assert_eq!(stored_paper(&app), "Custom 400x210");
    assert_eq!(page_mm(&app), (400.0, 210.0));

    type_edge(&mut app, PaperEdge::Height, "2");
    assert_eq!(stored_paper(&app), "Custom 400x10");

    type_edge(&mut app, PaperEdge::Width, "100000");
    assert_eq!(stored_paper(&app), "Custom 5080x10");

    let (size, orientation) = SheetSize::parse(&stored_paper(&app));
    assert!(size.is_recognized());
    let (w, h) = size.dimensions_mm(orientation);
    assert_eq!(orientation, SheetOrientation::Landscape);
    for edge in [w, h] {
        assert!(signex_output::parse_custom_edge_mm(&edge.to_string()).is_ok());
    }
}

/// Types `text` one character at a time into a custom edge box, then presses
/// Enter, the way the text input reports it.
fn type_edge(app: &mut Signex, edge: PaperEdge, text: &str) {
    let mut typed = String::new();
    for ch in text.chars() {
        typed.push(ch);
        panel(app, PanelMsg::CustomPaperEdgeTyping(edge, typed.clone()));
    }
    panel(app, PanelMsg::CommitCustomPaperEdge(edge));
}

/// #1082 — typing `400` keystroke by keystroke does not clamp the partial `4`
/// and `40` to the 10 mm minimum, writes nothing until Enter, and then lands
/// as one undo step.
#[test]
fn a_custom_edge_is_committed_once_on_enter_not_per_keystroke() {
    let mut app = app_with_title_block();
    panel(
        &mut app,
        PanelMsg::SetPaperSize("Custom 300x200".to_string()),
    );
    assert_eq!(stored_paper(&app), "Custom 300x200");

    for typed in ["4", "40", "400"] {
        panel(
            &mut app,
            PanelMsg::CustomPaperEdgeTyping(PaperEdge::Width, typed.to_string()),
        );
        assert_eq!(stored_paper(&app), "Custom 300x200", "nothing before Enter");
    }
    panel(&mut app, PanelMsg::CommitCustomPaperEdge(PaperEdge::Width));
    assert_eq!(stored_paper(&app), "Custom 400x200");

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(stored_paper(&app), "Custom 300x200", "one undo step");
}

// ---------------------------------------------------------------------------
// #1134 — per-sheet frame settings from Page Options
// ---------------------------------------------------------------------------

fn edit_frame(app: &mut Signex, edit: signex_app::panels::SheetFrameEdit) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::EditSheetFrame(
        edit,
    ))));
}

fn active_frame_settings(app: &Signex) -> signex_types::schematic::SheetFrameSettings {
    app.document_state
        .active_engine()
        .expect("active engine")
        .document()
        .frame
}

#[test]
fn page_options_zone_counts_redraw_the_zone_markers() {
    use signex_app::panels::SheetFrameEdit;
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");

    edit_frame(&mut app, SheetFrameEdit::HorizontalZones(4));
    edit_frame(&mut app, SheetFrameEdit::VerticalZones(2));

    let settings = active_frame_settings(&app);
    assert_eq!((settings.horizontal_zones, settings.vertical_zones), (4, 2));
    assert_eq!(app.document_state.panel_ctx.sheet_frame, settings);
    for zone in ["1", "4", "A", "B"] {
        assert!(text_at(&app, zone).is_some(), "zone {zone}");
    }
    for gone in ["5", "C"] {
        assert!(text_at(&app, gone).is_none(), "zone {gone}");
    }
}

#[test]
fn hiding_the_border_and_title_block_leaves_an_empty_frame_and_undo_restores_it() {
    use signex_app::panels::SheetFrameEdit;
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");
    let before = frame(&app).to_vec();

    edit_frame(&mut app, SheetFrameEdit::ShowTitleBlock(false));
    assert!(text_at(&app, "Power supply").is_none());
    assert!(text_at(&app, "1").is_some(), "the border stays");

    edit_frame(&mut app, SheetFrameEdit::ShowBorder(false));
    assert!(frame(&app).is_empty(), "{:?}", frame(&app));

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(
        active_frame_settings(&app),
        signex_types::schematic::SheetFrameSettings::default()
    );
    assert_eq!(frame(&app), before.as_slice());
}

/// The handler applies an edit to the engine's settings, not the panel's
/// copy: with the panel copy stale (two edits before a refresh), both land.
#[test]
fn a_frame_edit_applies_to_the_engine_settings_not_a_stale_panel_copy() {
    use signex_app::panels::SheetFrameEdit;
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");

    edit_frame(&mut app, SheetFrameEdit::HorizontalZones(4));
    // As if the panel had not been refreshed since the first edit.
    app.document_state.panel_ctx.sheet_frame = Default::default();
    edit_frame(&mut app, SheetFrameEdit::VerticalZones(2));

    let settings = active_frame_settings(&app);
    assert_eq!((settings.horizontal_zones, settings.vertical_zones), (4, 2));
}

#[test]
fn undo_and_redo_restore_the_settings_and_the_page_options_checkboxes() {
    use signex_app::panels::SheetFrameEdit;
    use signex_types::schematic::SheetFrameSettings;
    let mut app = app_with_title_block();
    set_paper(&mut app, "A4");
    edit_frame(&mut app, SheetFrameEdit::ShowTitleBlock(false));
    let edited = active_frame_settings(&app);
    assert!(!edited.show_title_block);

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Undo));
    assert_eq!(active_frame_settings(&app), SheetFrameSettings::default());
    assert_eq!(
        app.document_state.panel_ctx.sheet_frame,
        SheetFrameSettings::default(),
        "the checkbox follows undo"
    );
    assert!(text_at(&app, "Power supply").is_some());

    let _ = app.update(Message::Edit(signex_app::app::EditMsg::Redo));
    assert_eq!(active_frame_settings(&app), edited);
    assert_eq!(app.document_state.panel_ctx.sheet_frame, edited, "and redo");
    assert!(text_at(&app, "Power supply").is_none());
}
