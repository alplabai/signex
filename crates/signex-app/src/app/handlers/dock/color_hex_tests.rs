//! Mockup row 46: a colour typed in a row's hex field applies on Enter as
//! one undo step, refused text changes nothing, Esc restores.

use crate::app::contracts::DrawingFieldEdit as E;
use crate::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{ColorHexMsg, ColorRowKey, HexEntry, PanelMsg};
use signex_types::schematic::{
    FillType, Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind, StrokeColor,
};
use std::path::{Path, PathBuf};

/// A saved sheet with one rectangle, selected, with no colour of its own.
fn app() -> (Signex, PathBuf, uuid::Uuid) {
    let rect = SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(30.0, 20.0),
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    };
    let id = rect.uuid();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        drawings: vec![rect],
        ..Default::default()
    };
    let path = PathBuf::from("color-hex.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.mark_saved();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "color".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Drawing)];
    app.update_selection_info();
    (app, path, id)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn stroke(app: &Signex) -> Option<StrokeColor> {
    let engine = app.document_state.active_engine().expect("engine");
    match &engine.document().drawings[0] {
        SchDrawing::Rect { stroke_color, .. } => *stroke_color,
        other => panic!("not a rectangle: {other:?}"),
    }
}

fn typed(app: &mut Signex, id: uuid::Uuid, text: &str) {
    let key = ColorRowKey::DrawingStroke(id);
    panel(
        app,
        PanelMsg::ColorHex(ColorHexMsg::Typed(key, text.into())),
    );
}

/// What Enter in the drawing's hex field sends, built as the view builds it.
fn enter(app: &Signex, id: uuid::Uuid) -> PanelMsg {
    let ctx = &app.document_state.panel_ctx;
    let entry = HexEntry::of(ColorRowKey::DrawingStroke(id), ctx.color_hex_draft.as_ref());
    let current = stroke(app).map(|c| [c.r, c.g, c.b, c.a]);
    entry.left(current, &|[r, g, b, a]| {
        PanelMsg::UpdateDrawingEdit(E::StrokeColor(Some(StrokeColor { r, g, b, a })))
    })
}

fn can_undo(app: &Signex, path: &Path) -> bool {
    app.document_state.engines[path].can_undo()
}

#[test]
fn enter_applies_the_typed_colour_as_one_undo_step() {
    let (mut app, path, id) = app();
    for text in ["#f", "#fff7", "#fff7e0"] {
        typed(&mut app, id, text);
    }
    assert_eq!(stroke(&app), None, "typing is not an edit yet");
    assert!(!can_undo(&app, &path));
    let left = enter(&app, id);
    panel(&mut app, left.clone());
    let fff7e0 = StrokeColor {
        r: 0xFF,
        g: 0xF7,
        b: 0xE0,
        a: 255,
    };
    assert_eq!(stroke(&app), Some(fff7e0));
    assert_eq!(app.document_state.panel_ctx.color_hex_draft, None);
    // The blur that follows Enter finds no draft and adds nothing.
    panel(&mut app, left);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(stroke(&app), None, "one undo takes the whole edit back");
    assert!(!can_undo(&app, &path), "and it was the only step");
}

#[test]
fn refused_text_changes_nothing_and_stays_marked() {
    let (mut app, path, id) = app();
    typed(&mut app, id, "#FFF");
    let left = enter(&app, id);
    panel(&mut app, left);
    assert_eq!(stroke(&app), None);
    assert!(!can_undo(&app, &path));
    let draft = app.document_state.panel_ctx.color_hex_draft.as_ref();
    assert!(
        draft.is_some_and(|d| d.refused && d.text == "#FFF"),
        "{draft:?}"
    );
    typed(&mut app, id, "#FFF7");
    let draft = app.document_state.panel_ctx.color_hex_draft.as_ref();
    assert!(draft.is_some_and(|d| !d.refused), "typing clears the mark");
}

#[test]
fn esc_restores_and_the_blur_after_it_applies_nothing() {
    let (mut app, path, id) = app();
    typed(&mut app, id, "#123456");
    let blur = enter(&app, id);
    let key = ColorRowKey::DrawingStroke(id);
    panel(&mut app, PanelMsg::ColorHex(ColorHexMsg::Restored(key)));
    assert_eq!(app.document_state.panel_ctx.color_hex_draft, None);
    panel(&mut app, blur);
    assert_eq!(stroke(&app), None);
    assert!(!can_undo(&app, &path));
}

#[test]
fn a_draft_does_not_follow_another_selection() {
    let (mut app, _path, id) = app();
    typed(&mut app, id, "#123456");
    app.interaction_state.active_canvas_mut().selected.clear();
    app.update_selection_info();
    app.refresh_panel_ctx();
    assert_eq!(app.document_state.panel_ctx.color_hex_draft, None);
}
