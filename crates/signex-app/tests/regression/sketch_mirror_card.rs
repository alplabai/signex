//! mockup row 191 (DECISIONS 2026-11-23, 2026-11-24): the Mirror options
//! card. Picks collect and write nothing, OK writes one undo step, Cancel
//! writes nothing and mints no Point, Keep linked starts on and adds the
//! Symmetric constraints only while on, a mirrored pattern stays live with
//! it off, and the card works the same on a board sketch.

use std::path::{Path, PathBuf};

use signex_app::app::{
    BoardSketchMsg, ContextMenuMsg, FootprintDocument, Message, PcbMsg, Signex, TabContextAction,
    TabInfo, TabKind,
};
use signex_app::library::editor::footprint::sketch_dispatch::apply_sketch_edit;
use signex_app::library::editor::footprint::sketch_mode::SketchEdit;
use signex_app::library::editor::footprint::sketch_mode::mirror_card::{
    MirrorCard, MirrorCardMsg, MirrorObject,
};
use signex_app::library::editor::footprint::sketch_mode::value_edit::SketchCanvasMsg;
use signex_app::library::editor::footprint::{EditorMode, SketchTool};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::constraint::ConstraintKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use crate::board_sketch::{self, Fixture};
use crate::menu_support::menu_row;

/// A vertical centerline at x = 0 and a Line from (2, 0) to (4, 1) to
/// its right.
struct Drawing {
    sketch: SketchData,
    centerline: SketchEntityId,
    line: SketchEntityId,
    ends: [SketchEntityId; 2],
}

fn drawing() -> Drawing {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let c0 = add(EntityKind::Point { x: 0.0, y: -5.0 });
    let c1 = add(EntityKind::Point { x: 0.0, y: 5.0 });
    let centerline = add(EntityKind::Line { start: c0, end: c1 });
    let a = add(EntityKind::Point { x: 2.0, y: 0.0 });
    let b = add(EntityKind::Point { x: 4.0, y: 1.0 });
    let line = add(EntityKind::Line { start: a, end: b });
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == centerline)
        .unwrap()
        .centerline = true;
    Drawing {
        sketch,
        centerline,
        line,
        ends: [a, b],
    }
}

/// `sketch` open in a footprint tab in sketch mode with Mirror armed.
fn open(stem: &str, sketch: SketchData) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(sketch);
    let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Mirror;
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    editor_mut(&mut app, &path)
        .with_parts(|state, fp| apply_sketch_edit(state, fp, SketchEdit::ForceRebuild))
        .expect("the drawing solves");
    (app, path)
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

fn editor_mut<'a>(app: &'a mut Signex, path: &PathBuf) -> &'a mut FootprintDocument {
    app.document_state.footprint_editors.get_mut(path).unwrap()
}

fn sketch<'a>(app: &'a Signex, path: &PathBuf) -> &'a SketchData {
    editor(app, path).primitive().sketch.as_ref().unwrap()
}

fn send(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

fn click(app: &mut Signex, path: &Path, x_mm: f64, y_mm: f64) {
    send(
        app,
        path,
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: None,
        },
    );
}

fn card_msg(msg: MirrorCardMsg) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchMirrorCard(msg)
}

fn answer(app: &mut Signex, path: &Path, msg: MirrorCardMsg) {
    send(app, path, card_msg(msg));
}

fn arm(app: &mut Signex, path: &Path) {
    let msg = FootprintEditorMsg::SketchSetTool(SketchTool::Mirror);
    send(app, path, msg);
}

fn card<'a>(app: &'a Signex, path: &PathBuf) -> &'a MirrorCard {
    app.library
        .mirror_cards
        .get(path)
        .expect("the card is open")
}

fn is_open(app: &Signex, path: &PathBuf) -> bool {
    app.library.mirror_cards.contains_key(path)
}

/// The copy of the Line from (2, 0) to (4, 1) about x = 0.
fn has_copy(sketch: &SketchData) -> bool {
    point_at(sketch, -2.0, 0.0) && point_at(sketch, -4.0, 1.0)
}

fn symmetric_pairs(sketch: &SketchData) -> usize {
    sketch
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::SymmetricAboutLine { .. }))
        .count()
}

fn point_at(sketch: &SketchData, x: f64, y: f64) -> bool {
    sketch.entities.iter().any(|e| match e.kind {
        EntityKind::Point { x: px, y: py } => (px - x).abs() < 1e-6 && (py - y).abs() < 1e-6,
        _ => false,
    })
}

/// Pick the Line and the centerline on the footprint host.
fn picked(stem: &str) -> (Signex, PathBuf, Drawing) {
    let d = drawing();
    let (mut app, path) = open(stem, d.sketch.clone());
    click(&mut app, &path, 3.0, 0.5);
    click(&mut app, &path, 0.05, 2.0);
    (app, path, d)
}

#[test]
fn picks_collect_and_ok_writes_one_undo_step_with_symmetric_constraints() {
    let (mut app, path, d) = picked("mirror-ok");
    let before = sketch(&app, &path).clone();
    let undo_before = editor(&app, &path).history.undo_len();
    let picked = card(&app, &path);
    assert_eq!(picked.objects, vec![MirrorObject::Entity(d.line)]);
    assert_eq!(picked.line, Some(d.centerline));
    assert!(picked.keep_linked, "Keep linked starts on");
    assert_eq!(sketch(&app, &path), &before, "picks write nothing");
    assert!(!editor(&app, &path).is_dirty());

    answer(&mut app, &path, MirrorCardMsg::Confirmed);

    let sk = sketch(&app, &path);
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before + 1);
    assert!(editor(&app, &path).is_dirty());
    let added = sk.entities.len() - before.entities.len();
    assert_eq!(added, 3, "2 Points, 1 Line");
    assert!(has_copy(sk));
    assert_eq!(symmetric_pairs(sk), 2);
    for end in d.ends {
        assert!(sk.constraints.iter().any(|c| matches!(
            c.kind,
            ConstraintKind::SymmetricAboutLine { p1, line, .. } if p1 == end && line == d.centerline
        )));
    }
    assert_eq!(editor(&app, &path).state.active_tool, SketchTool::Select);
    assert!(!is_open(&app, &path), "the card closes");
}

#[test]
fn keep_linked_off_adds_no_symmetric_constraint() {
    let (mut app, path, _) = picked("mirror-free");
    answer(&mut app, &path, MirrorCardMsg::KeepLinkedToggled(false));
    assert!(!card(&app, &path).keep_linked);

    answer(&mut app, &path, MirrorCardMsg::Confirmed);

    let sk = sketch(&app, &path);
    assert!(has_copy(sk), "the copy is made");
    assert_eq!(symmetric_pairs(sk), 0, "a free copy");
}

#[test]
fn cancel_writes_nothing_mints_no_point_and_disarms() {
    let (mut app, path, _) = picked("mirror-cancel");
    let before = sketch(&app, &path).clone();
    let undo_before = editor(&app, &path).history.undo_len();

    answer(&mut app, &path, MirrorCardMsg::Cancelled);

    assert_eq!(sketch(&app, &path), &before, "no Point, no copy");
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before);
    assert!(!editor(&app, &path).is_dirty());
    assert_eq!(editor(&app, &path).state.active_tool, SketchTool::Select);
    assert!(!is_open(&app, &path));
}

#[test]
fn keep_linked_starts_on_each_time_the_card_opens() {
    let (mut app, path, _) = picked("mirror-reopen");
    answer(&mut app, &path, MirrorCardMsg::KeepLinkedToggled(false));
    answer(&mut app, &path, MirrorCardMsg::Cancelled);

    arm(&mut app, &path);
    click(&mut app, &path, 20.0, 20.0);

    let reopened = card(&app, &path);
    assert!(reopened.keep_linked);
    assert!(reopened.objects.is_empty() && reopened.line.is_none());
}

#[test]
fn ok_without_a_line_writes_nothing() {
    let d = drawing();
    let (mut app, path) = open("mirror-no-line", d.sketch);
    let before = sketch(&app, &path).clone();
    click(&mut app, &path, 3.0, 0.5);
    let undo_before = editor(&app, &path).history.undo_len();

    answer(&mut app, &path, MirrorCardMsg::Confirmed);

    assert_eq!(sketch(&app, &path), &before);
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before);
    let objects = &card(&app, &path).objects;
    assert_eq!(objects, &vec![MirrorObject::Entity(d.line)]);
}

/// A pad on the Line's first end and a two-long row of it; the row's id.
fn pad_row(d: &mut Drawing) -> ArrayId {
    let pad = d.ends[0];
    d.sketch
        .entities
        .iter_mut()
        .find(|e| e.id == pad)
        .unwrap()
        .pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    let row = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: pad,
            count_expr: "2".into(),
            dx_expr: "0mm".into(),
            dy_expr: "2mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    };
    let row_id = row.id;
    d.sketch.arrays.push(row);
    row_id
}

#[test]
fn a_mirrored_pattern_stays_live_with_keep_linked_off() {
    let mut d = drawing();
    let row_id = pad_row(&mut d);
    let (mut app, path) = open("mirror-pattern-free", d.sketch);
    send(
        &mut app,
        &path,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::PatternSelected(Some(row_id))),
    );
    click(&mut app, &path, 0.05, 2.0);
    answer(&mut app, &path, MirrorCardMsg::KeepLinkedToggled(false));

    answer(&mut app, &path, MirrorCardMsg::Confirmed);

    let sk = sketch(&app, &path);
    assert_eq!(sk.pattern_mirrors.len(), 1, "a live mirror, not copies");
    assert_eq!(sk.pattern_mirrors[0].pattern, row_id);
    assert_eq!(sk.pattern_mirrors[0].line, d.centerline);
}

/// #2859 (decision 16): a glyph click selects the row and its source pad
/// Point; Mirror armed next mirrors the row alone, with no bare copy of
/// the source stacked on the mirrored first instance.
#[test]
fn a_pattern_picked_by_its_glyph_is_mirrored_without_its_source() {
    let mut d = drawing();
    let row_id = pad_row(&mut d);
    let (mut app, path) = open("mirror-pattern-glyph", d.sketch);
    editor_mut(&mut app, &path).state.active_tool = SketchTool::Select;
    let glyph = SketchCanvasMsg::PatternSelected(Some(row_id));
    send(&mut app, &path, FootprintEditorMsg::SketchCanvas(glyph));
    assert_eq!(editor(&app, &path).state.selected_sketch, Some(d.ends[0]));
    let entities = sketch(&app, &path).entities.len();

    arm(&mut app, &path);
    click(&mut app, &path, 0.05, 2.0);
    assert_eq!(card(&app, &path).objects, vec![MirrorObject::Pattern(row_id)]);
    answer(&mut app, &path, MirrorCardMsg::Confirmed);

    let sk = sketch(&app, &path);
    assert_eq!(sk.pattern_mirrors.len(), 1);
    assert_eq!(sk.entities.len(), entities, "no copy of the source");
}

/// #2859 (decision 16): a pattern picked by its glyph and then left for
/// another entity is not taken by the card.
#[test]
fn a_pattern_left_for_another_entity_is_not_mirrored() {
    let mut d = drawing();
    let row_id = pad_row(&mut d);
    let (mut app, path) = open("mirror-pattern-stale", d.sketch);
    editor_mut(&mut app, &path).state.active_tool = SketchTool::Select;
    let glyph = SketchCanvasMsg::PatternSelected(Some(row_id));
    send(&mut app, &path, FootprintEditorMsg::SketchCanvas(glyph));
    let select = FootprintEditorMsg::SketchSelect {
        id: Some(d.line),
        shift: false,
    };
    send(&mut app, &path, select);

    arm(&mut app, &path);
    click(&mut app, &path, 0.05, 2.0);

    assert_eq!(card(&app, &path).objects, vec![MirrorObject::Entity(d.line)]);
}

// ─── Board host ─────────────────────────────────────────────────────────────

fn board_sketch_of(f: &Fixture) -> SketchData {
    f.app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("sketch mode")
        .working_sketch()
}

fn board_undo_len(f: &Fixture) -> usize {
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    session.editor.history.undo_len()
}

/// A board in sketch mode over [`drawing`], with Mirror armed, the Line
/// and the centerline picked.
fn board_picked() -> (Fixture, Drawing, SketchData) {
    let mut f = board_sketch::open_board();
    board_sketch::send(&mut f, menu_row("board_sketch"));
    assert!(board_sketch::in_sketch(&f));
    let d = drawing();
    let session = f.app.interaction_state.board_sketch.as_mut().unwrap();
    session.editor.primitive_mut().sketch = Some(d.sketch.clone());
    let arm = FootprintEditorMsg::SketchSetTool(SketchTool::Mirror);
    board_sketch::edit(&mut f, arm);
    let before = board_sketch_of(&f);
    board_sketch::click(&mut f, 3.0, 0.5);
    board_sketch::click(&mut f, 0.05, 2.0);
    (f, d, before)
}

#[test]
fn the_card_works_on_a_board_sketch() {
    let (mut f, d, before) = board_picked();
    let undo_before = board_undo_len(&f);
    let picked = f.app.library.mirror_cards.get(&f.path).expect("the card");
    assert_eq!(picked.objects, vec![MirrorObject::Entity(d.line)]);
    assert_eq!(picked.line, Some(d.centerline));
    assert_eq!(board_sketch_of(&f), before, "picks write nothing");

    board_sketch::edit(&mut f, card_msg(MirrorCardMsg::Confirmed));

    let sk = board_sketch_of(&f);
    assert!(has_copy(&sk));
    assert_eq!(symmetric_pairs(&sk), 2);
    assert_eq!(board_undo_len(&f), undo_before + 1);
}

#[test]
fn cancel_on_a_board_sketch_writes_nothing() {
    let (mut f, _, before) = board_picked();
    let undo_before = board_undo_len(&f);

    board_sketch::edit(&mut f, card_msg(MirrorCardMsg::Cancelled));

    assert_eq!(board_sketch_of(&f), before, "no Point, no copy");
    assert_eq!(board_undo_len(&f), undo_before);
    assert!(!f.app.library.mirror_cards.contains_key(&f.path));
}

// ─── Lifetime (#2859) ───────────────────────────────────────────────────────

/// Closing a footprint tab with Mirror armed drops its card.
#[test]
fn closing_the_tab_drops_its_mirror_card() {
    let (mut app, path, _) = picked("mirror-tab-close");
    assert!(is_open(&app, &path));

    let close = ContextMenuMsg::TabAction(TabContextAction::Close(0));
    let task = app.update(Message::ContextMenu(close));
    crate::drive::drive(&mut app, task);

    assert!(app.document_state.tabs.iter().all(|t| t.path != path));
    assert!(!is_open(&app, &path));
}

/// Leaving the board sketch with Mirror armed drops the board's card.
#[test]
fn ending_the_board_sketch_drops_its_mirror_card() {
    let (mut f, _, _) = board_picked();
    assert!(f.app.library.mirror_cards.contains_key(&f.path));

    for msg in [
        BoardSketchMsg::DiscardRequested,
        BoardSketchMsg::DiscardConfirmed,
    ] {
        board_sketch::send(&mut f, Message::Pcb(PcbMsg::Sketch(msg)));
    }

    assert!(!board_sketch::in_sketch(&f));
    assert!(!f.app.library.mirror_cards.contains_key(&f.path));
}
