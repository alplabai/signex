//! #1079 — the canvas draws the sheet frame, zone markers and title block,
//! lowered by the same function the PDF uses and filled from the sheet's
//! title block, and it follows the sheet's paper size.

use signex_app::app::{Message, Signex, TabInfo, TabKind, WindowMsg};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
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
    assert!(text_at(&app, "Rev: B").is_some());
    assert!(text_at(&app, "Sheet 1 of 1").is_some());
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
    // A4 landscape is 297 x 210 mm; the block's title sits 4 mm into a
    // 180 x 40 mm block in the bottom-right of the 10 mm-margin frame.
    assert!((a4_x - 111.0).abs() < 1e-6 && (a4_y - 164.0).abs() < 1e-6);

    set_paper(&mut app, "A3");
    let (a3_x, a3_y) = text_at(&app, "Power supply").unwrap();
    assert!((a3_x - 234.0).abs() < 1e-6 && (a3_y - 251.0).abs() < 1e-6);
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
    assert!(text_at(&app, "Sheet 1 of 2").is_some(), "{:?}", frame(&app));
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

    panel(&mut app, PanelMsg::SetCustomPaperWidth(400.0));
    assert_eq!(stored_paper(&app), "Custom 400x210");
    assert_eq!(page_mm(&app), (400.0, 210.0));

    panel(&mut app, PanelMsg::SetCustomPaperHeight(2.0));
    assert_eq!(stored_paper(&app), "Custom 400x10");

    panel(&mut app, PanelMsg::SetCustomPaperWidth(100_000.0));
    assert_eq!(stored_paper(&app), "Custom 5080x10");

    let (size, orientation) = SheetSize::parse(&stored_paper(&app));
    assert!(size.is_recognized());
    let (w, h) = size.dimensions_mm(orientation);
    assert_eq!(orientation, SheetOrientation::Landscape);
    for edge in [w, h] {
        assert!(signex_output::parse_custom_edge_mm(&edge.to_string()).is_ok());
    }
}
