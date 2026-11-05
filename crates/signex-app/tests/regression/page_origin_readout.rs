//! #2034 - Page Origin is display only: Properties > Position and typed
//! coordinates read Y from the chosen corner, the stored point never moves.
//! The default (no origin in the file) is Lower Left.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{DrawingFieldId, PageOrigin, PanelMsg, PaperEdge};
use signex_types::schematic::{
    Label, LabelType, Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind,
};
use std::path::PathBuf;

/// A4 landscape: 297 x 210 mm.
const PAPER_H: f64 = 210.0;

fn app_with(label: Label, line: SchDrawing) -> (Signex, PathBuf) {
    let path = PathBuf::from("origin.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        labels: vec![label],
        drawings: vec![line],
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "origin".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    (app, path)
}

fn label(id: uuid::Uuid, x: f64, y: f64) -> Label {
    Label {
        uuid: id,
        text: "NET".into(),
        position: Point::new(x, y),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: Default::default(),
        justify_v: Default::default(),
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn line(id: uuid::Uuid) -> SchDrawing {
    SchDrawing::Line {
        uuid: id,
        start: Point::new(5.0, 40.0),
        end: Point::new(25.0, 40.0),
        width: 0.0,
        stroke_color: None,
        locked: false,
    }
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn position_row(app: &Signex) -> String {
    app.document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Position")
        .map(|(_, v)| v.clone())
        .expect("a Position row")
}

/// Select `item`, then show the sheet with `origin`. The origin change
/// refreshes the Position rows, so it goes through Upper Left first.
fn select(app: &mut Signex, item: SelectedItem, origin: PageOrigin) {
    app.interaction_state.active_canvas_mut().selected = vec![item];
    panel(app, PanelMsg::SetPageOrigin(PageOrigin::UpperLeft));
    panel(app, PanelMsg::SetPageOrigin(origin));
}

#[test]
fn the_position_row_reads_y_from_the_chosen_corner() {
    let id = uuid::Uuid::now_v7();
    let (mut app, path) = app_with(label(id, 10.0, 30.0), line(uuid::Uuid::now_v7()));
    let item = SelectedItem::new(id, SelectedKind::Label);

    select(&mut app, item, PageOrigin::LowerLeft);
    assert_eq!(position_row(&app), format!("10.00, {:.2}", PAPER_H - 30.0));

    select(&mut app, item, PageOrigin::UpperLeft);
    assert_eq!(position_row(&app), "10.00, 30.00");
    // Display only: the stored point never moved.
    let doc = app.document_state.engines[&path].document();
    assert_eq!(doc.labels[0].position, Point::new(10.0, 30.0));
}

/// Typing the Y the panel shows puts the item back on the same stored point.
#[test]
fn a_typed_y_is_read_from_the_chosen_corner() {
    for origin in [PageOrigin::LowerLeft, PageOrigin::UpperLeft] {
        let id = uuid::Uuid::now_v7();
        let (mut app, path) = app_with(label(uuid::Uuid::now_v7(), 0.0, 0.0), line(id));
        select(
            &mut app,
            SelectedItem::new(id, SelectedKind::Drawing),
            origin,
        );
        let shown = origin.map_y(70.0, PAPER_H);
        panel(
            &mut app,
            PanelMsg::DrawingFieldTyping(DrawingFieldId::LineStartY, format!("{shown}")),
        );
        let doc = app.document_state.engines[&path].document();
        let SchDrawing::Line { start, .. } = &doc.drawings[0] else {
            panic!("a line");
        };
        assert!((start.y - 70.0).abs() < 1e-9, "{origin:?}: {start:?}");
    }
}

/// The Position row, if one is shown, agrees with the sheet's current paper:
/// a stale row would move the item when its Y is typed back.
fn position_matches_paper(app: &Signex, path: &PathBuf, raw_y: f64) {
    let doc = app.document_state.engines[path].document();
    let want = format!("10.00, {:.2}", doc.page_y(raw_y));
    if let Some((_, shown)) = app
        .document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Position")
    {
        assert_eq!(shown, &want, "paper {}", doc.paper_size);
    }
}

/// Review of #2034: under Lower Left the shown Y follows a paper change
/// (named size, custom edge) and its undo.
#[test]
fn the_position_row_follows_a_paper_change() {
    let id = uuid::Uuid::now_v7();
    let (mut app, path) = app_with(label(id, 10.0, 30.0), line(uuid::Uuid::now_v7()));
    select(
        &mut app,
        SelectedItem::new(id, SelectedKind::Label),
        PageOrigin::LowerLeft,
    );
    assert_eq!(position_row(&app), "10.00, 180.00", "A4");

    panel(&mut app, PanelMsg::SetPaperSize("A3".into()));
    assert_eq!(position_row(&app), "10.00, 267.00", "A3");

    panel(&mut app, PanelMsg::SetPaperSize("User:200x150".into()));
    panel(
        &mut app,
        PanelMsg::CustomPaperEdgeTyping(PaperEdge::Height, "100".into()),
    );
    panel(&mut app, PanelMsg::CommitCustomPaperEdge(PaperEdge::Height));
    assert_eq!(position_row(&app), "10.00, 70.00", "custom 100 mm high");

    for _ in 0..3 {
        let _ = app.update(Message::Edit(EditMsg::Undo));
        position_matches_paper(&app, &path, 30.0);
    }
    for _ in 0..3 {
        let _ = app.update(Message::Edit(EditMsg::Redo));
        position_matches_paper(&app, &path, 30.0);
    }
}

fn buffered(app: &Signex) -> bool {
    !app.document_state.panel_ctx.drawing_edit_buf.is_empty()
}

/// Review of #2034: a typed Y box is read under one origin and paper, so a
/// change of either (or its undo) drops the typed text and the box shows the
/// stored value again.
#[test]
fn a_typed_y_box_drops_its_text_on_an_origin_or_paper_change() {
    let id = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(label(uuid::Uuid::now_v7(), 0.0, 0.0), line(id));
    select(
        &mut app,
        SelectedItem::new(id, SelectedKind::Drawing),
        PageOrigin::LowerLeft,
    );
    let type_y = |app: &mut Signex| {
        panel(
            app,
            PanelMsg::DrawingFieldTyping(DrawingFieldId::LineStartY, "150".into()),
        );
        assert!(buffered(app), "the typed text is kept while typing");
    };

    type_y(&mut app);
    panel(&mut app, PanelMsg::SetPageOrigin(PageOrigin::UpperLeft));
    assert!(!buffered(&app), "origin change");

    type_y(&mut app);
    panel(&mut app, PanelMsg::SetPaperSize("A3".into()));
    assert!(!buffered(&app), "paper change");

    type_y(&mut app);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(!buffered(&app), "undo of the paper change");
}
