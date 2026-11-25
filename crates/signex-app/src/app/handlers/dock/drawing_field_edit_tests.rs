//! #2767, #2768: a drawing's number box previews while typed, commits as one
//! undo step, restores on Esc and refuses what it cannot store.

use crate::app::{EditMsg, InputMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{DrawingFieldId as F, PanelKind, PanelMsg};
use signex_types::schematic::{
    FillType, Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind,
};
use std::path::{Path, PathBuf};

fn rect_shape(right: f64) -> SchDrawing {
    SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, 10.0),
        end: Point::new(right, 20.0),
        width: 0.2,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    }
}

/// A saved sheet with two rectangles, the first selected.
fn app() -> (Signex, PathBuf) {
    let first = rect_shape(30.0);
    let id = first.uuid();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        drawings: vec![first, rect_shape(50.0)],
        ..Default::default()
    };
    let path = PathBuf::from("drawing-field-edit.snxsch");
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
        title: "drawing".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    select(&mut app, id);
    (app, path)
}

fn select(app: &mut Signex, id: uuid::Uuid) {
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Drawing)];
    app.update_selection_info();
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn typing(app: &mut Signex, text: &str) {
    panel(app, PanelMsg::DrawingFieldTyping(F::RectWidth, text.into()));
}

fn width(app: &Signex, index: usize) -> f64 {
    let engine = app.document_state.active_engine().expect("engine");
    match &engine.document().drawings[index] {
        SchDrawing::Rect { start, end, .. } => (end.x - start.x).abs(),
        other => panic!("not a rectangle: {other:?}"),
    }
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn esc(app: &mut Signex) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
}

fn can_undo(app: &Signex, path: &Path) -> bool {
    app.document_state.engines[path].can_undo()
}

fn buffered(app: &Signex) -> Option<&str> {
    let buffer = &app.document_state.panel_ctx.drawing_edit_buf;
    buffer.get(&F::RectWidth).map(String::as_str)
}

#[test]
fn a_preview_shows_on_the_sheet_but_is_not_an_edit_yet() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    assert_eq!(width(&app, 0), 45.0);
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(!can_undo(&app, &path));
}

#[test]
fn enter_commits_the_whole_edit_as_one_step() {
    let (mut app, path) = app();
    for text in ["4", "45", "45.5"] {
        typing(&mut app, text);
    }
    panel(&mut app, PanelMsg::DrawingFieldLeft(F::RectWidth));
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(buffered(&app), None, "the box shows the stored value");
    undo(&mut app);
    assert_eq!(width(&app, 0), 20.0);
    assert!(!can_undo(&app, &path));
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn esc_restores_the_old_value_with_no_step() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    esc(&mut app);
    assert_eq!(width(&app, 0), 20.0);
    assert_eq!(buffered(&app), None);
    assert!(!can_undo(&app, &path));
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn switching_the_selection_commits_the_edit() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    let other = app.document_state.engines[&path].document().drawings[1].uuid();
    select(&mut app, other);
    panel(&mut app, PanelMsg::CancelWireColorPicker);
    assert_eq!(width(&app, 0), 45.0);
    assert!(can_undo(&app, &path));
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(buffered(&app), None, "no text follows to the other shape");
}

#[test]
fn closing_properties_commits_the_edit() {
    let (mut app, path) = app();
    app.document_state.dock.show_panel(PanelKind::Properties);
    typing(&mut app, "45");
    app.document_state.dock.close_panel(PanelKind::Properties);
    panel(&mut app, PanelMsg::CancelWireColorPicker);
    assert_eq!(width(&app, 0), 45.0);
    assert!(can_undo(&app, &path));
}

#[test]
fn a_save_commit_lands_the_typed_value() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    app.commit_title_block_drafts_where(|sheet| sheet == path);
    assert!(can_undo(&app, &path));
    assert!(app.document_state.is_document_dirty(&path));
}

#[test]
fn refused_text_stays_marked_and_puts_the_old_value_back() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    typing(&mut app, "45x");
    assert_eq!(width(&app, 0), 45.0, "the last good preview while typing");
    assert_eq!(buffered(&app), Some("45x"));
    assert!(F::RectWidth.refuses("45x"));

    panel(&mut app, PanelMsg::DrawingFieldLeft(F::RectWidth));
    assert_eq!(width(&app, 0), 20.0, "a typo never commits the preview");
    assert_eq!(buffered(&app), Some("45x"), "kept to be fixed");
    assert!(!can_undo(&app, &path));
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn fixing_a_typo_goes_on_as_the_same_edit() {
    let (mut app, path) = app();
    for text in ["45", "45x", "46"] {
        typing(&mut app, text);
    }
    panel(&mut app, PanelMsg::DrawingFieldLeft(F::RectWidth));
    assert_eq!(width(&app, 0), 46.0);
    undo(&mut app);
    assert_eq!(width(&app, 0), 20.0);
    assert!(!can_undo(&app, &path));
}

#[test]
fn esc_on_a_typo_restores_the_value_from_before_the_edit() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    typing(&mut app, "45x");
    esc(&mut app);
    assert_eq!(width(&app, 0), 20.0);
    assert_eq!(buffered(&app), None);
    assert!(!can_undo(&app, &path));
}

#[test]
fn undo_on_a_typo_only_takes_back_the_preview() {
    let (mut app, path) = app();
    typing(&mut app, "30");
    panel(&mut app, PanelMsg::DrawingFieldLeft(F::RectWidth));
    typing(&mut app, "45");
    typing(&mut app, "45x");
    undo(&mut app);
    assert_eq!(width(&app, 0), 30.0, "the earlier edit stays");
    assert!(can_undo(&app, &path));
}

#[test]
fn switching_the_selection_on_a_typo_does_not_commit_the_preview() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    typing(&mut app, "45x");
    let other = app.document_state.engines[&path].document().drawings[1].uuid();
    select(&mut app, other);
    panel(&mut app, PanelMsg::CancelWireColorPicker);
    assert_eq!(width(&app, 0), 20.0);
    assert!(!can_undo(&app, &path));
    assert_eq!(buffered(&app), None, "no text follows to the other shape");
}

/// Mockup row 48: a pick in the Color popover closes it and is one undo step.
#[test]
fn a_colour_pick_closes_the_popover_as_one_step() {
    use crate::app::contracts::DrawingFieldEdit;
    use crate::panels::WireColorPicker;
    use signex_types::schematic::StrokeColor;
    let (mut app, path) = app();
    panel(&mut app, PanelMsg::ToggleWireColorPicker);
    let red = StrokeColor {
        r: 0xC0,
        g: 0x39,
        b: 0x2B,
        a: 255,
    };
    let edit = DrawingFieldEdit::StrokeColor(Some(red));
    panel(&mut app, PanelMsg::UpdateDrawingEdit(edit));
    let picker = app.document_state.panel_ctx.wire_color_picker;
    assert_eq!(picker, WireColorPicker::Closed);
    undo(&mut app);
    assert!(!can_undo(&app, &path));
}

#[test]
fn an_emptied_box_left_shows_the_stored_value_again() {
    let (mut app, path) = app();
    typing(&mut app, "45");
    typing(&mut app, "");
    panel(&mut app, PanelMsg::DrawingFieldLeft(F::RectWidth));
    assert_eq!(width(&app, 0), 20.0);
    assert_eq!(buffered(&app), None);
    assert!(!can_undo(&app, &path));
}

#[test]
fn what_each_box_can_store() {
    for bad in ["-5", "0", "abc", "nan", "inf"] {
        assert!(F::RectWidth.refuses(bad), "{bad}");
        assert!(F::CircleRadius.refuses(bad), "{bad}");
    }
    assert!(F::RectBorder.reads("0"), "a hairline border");
    assert!(F::RectBorder.refuses("-1"));
    assert!(F::LineStartX.reads("-5"), "a position may be negative");
    for mid_edit in ["", "-"] {
        assert!(!F::RectWidth.refuses(mid_edit), "{mid_edit:?}");
    }
}
