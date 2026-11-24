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
    let (app, path, ids) = app_with_rects(1);
    (app, path, ids[0])
}

/// A saved sheet with `count` rectangles without a colour of their own, the
/// first one selected.
fn app_with_rects(count: usize) -> (Signex, PathBuf, Vec<uuid::Uuid>) {
    let drawings: Vec<SchDrawing> = (0..count)
        .map(|i| {
            let x = 10.0 + 40.0 * i as f64;
            SchDrawing::Rect {
                uuid: uuid::Uuid::now_v7(),
                start: Point::new(x, 10.0),
                end: Point::new(x + 20.0, 20.0),
                width: 0.2,
                fill: FillType::None,
                stroke_color: None,
                locked: false,
            }
        })
        .collect();
    let ids: Vec<uuid::Uuid> = drawings.iter().map(SchDrawing::uuid).collect();
    let id = ids[0];
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        drawings,
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
    (app, path, ids)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn stroke(app: &Signex) -> Option<StrokeColor> {
    stroke_of(app, 0)
}

/// The colour of the sheet's `index`th rectangle.
fn stroke_of(app: &Signex, index: usize) -> Option<StrokeColor> {
    let engine = app.document_state.active_engine().expect("engine");
    match &engine.document().drawings[index] {
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

#[test]
fn a_blur_after_another_selection_colours_nothing() {
    let (mut app, path, ids) = app_with_rects(2);
    typed(&mut app, ids[0], "#123456");
    let blur = enter(&app, ids[0]);
    // The other drawing is selected before the blur lands; the panel has
    // not been rebuilt yet, so the draft is still held.
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(ids[1], SelectedKind::Drawing)];
    app.update_selection_info();
    assert!(app.document_state.panel_ctx.color_hex_draft.is_some());
    panel(&mut app, blur);
    assert_eq!(stroke_of(&app, 1), None, "the other keeps its colour");
    assert_eq!(stroke_of(&app, 0), None);
    assert_eq!(app.document_state.panel_ctx.color_hex_draft, None);
    assert!(!can_undo(&app, &path), "no undo step");
}

/// How often this thread ran `finish_update`.
fn finish_updates() -> usize {
    crate::app::runtime::FINISH_UPDATES.with(std::cell::Cell::get)
}

#[test]
fn a_hex_message_finishes_the_update_once() {
    let (mut app, _path, id) = app();
    let key = ColorRowKey::DrawingStroke(id);
    let before = finish_updates();
    let typed = PanelMsg::ColorHex(ColorHexMsg::Typed(key, "#123456".into()));
    let _ = app.handle_dock_message(DockMessage::Panel(typed));
    assert_eq!(finish_updates() - before, 1, "a keystroke");
    let left = enter(&app, id);
    let pick = match &left {
        PanelMsg::ColorHex(ColorHexMsg::Left {
            apply: Some(pick), ..
        }) => (**pick).clone(),
        other => panic!("no pick to apply: {other:?}"),
    };
    let before = finish_updates();
    let _ = app.handle_dock_message(DockMessage::Panel(left));
    assert!(stroke(&app).is_some(), "the pick applied");
    let via_hex = finish_updates() - before;
    // The same pick from the row's popover, on a fresh sheet.
    let (mut direct, _path, _id) = self::app();
    let before = finish_updates();
    let _ = direct.handle_dock_message(DockMessage::Panel(pick));
    assert!(stroke(&direct).is_some(), "the popover's pick applied");
    let alone = finish_updates() - before;
    assert_eq!(via_hex, alone, "the hex field adds no second finish");
}
