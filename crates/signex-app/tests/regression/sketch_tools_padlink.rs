//! mockup row 187 / mockup row 191 / mockup row 192 — the Modify tools Chamfer, Extend and Linear
//! Pattern, Mirror about a centerline, and the pad link: Edit in Sketch
//! focuses the pad's driving profile, Back to pad / Jump to pad return to
//! it.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::driving_profile;
use signex_app::library::editor::footprint::{
    EditorMode, PadLinkMsg, PadLinkOffer, SketchTool, ToolPending,
};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Pad};
use signex_sketch::SketchData;
use signex_sketch::array::ArrayKind;
use signex_sketch::attr::PadAttr;
use signex_sketch::constraint::ConstraintKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use std::path::{Path, PathBuf};

/// A sketch under construction: one plane, Points and Lines by position.
struct Sketch {
    plane: PlaneId,
    data: SketchData,
}

impl Sketch {
    fn new() -> Self {
        let plane = PlaneId::new();
        let data = SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            ..SketchData::default()
        };
        Self { plane, data }
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Point { x, y }));
        id
    }

    fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Line { start, end }));
        id
    }
}

/// `fp` open in a footprint tab, in sketch mode with `tool` armed.
fn open(stem: &str, fp: Footprint, tool: SketchTool) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = tool;
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn with_sketch(sketch: Sketch) -> Footprint {
    let mut fp = Footprint::empty("t");
    fp.sketch = Some(sketch.data);
    fp
}

fn edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

fn click(app: &mut Signex, path: &Path, x_mm: f64, y_mm: f64, snap: Option<SketchEntityId>) {
    edit(
        app,
        path,
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: snap,
        },
    );
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

fn kind_of(sketch: &SketchData, id: SketchEntityId) -> EntityKind {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.kind.clone())
        .unwrap()
}

fn xy(sketch: &SketchData, id: SketchEntityId) -> (f64, f64) {
    match kind_of(sketch, id) {
        EntityKind::Point { x, y } => (x, y),
        other => panic!("not a point: {other:?}"),
    }
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
}

fn points(sketch: &SketchData) -> usize {
    sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .count()
}

/// An L corner at the origin: a Line along +X to (10, 0) and one along +Y
/// to (0, 10). Returns the sketch, (along X, along Y).
fn corner() -> (Sketch, SketchEntityId, SketchEntityId) {
    let mut s = Sketch::new();
    let c = s.point(0.0, 0.0);
    let x = s.point(10.0, 0.0);
    let y = s.point(0.0, 10.0);
    let along_x = s.line(x, c);
    let along_y = s.line(c, y);
    (s, along_x, along_y)
}

// ─── Chamfer (mockup row 187) ───────────────────────────────────────────────

#[test]
fn chamfer_cuts_both_lines_and_joins_the_cut_ends() {
    let (s, along_x, along_y) = corner();
    let (mut app, path) = open("chamfer", with_sketch(s), SketchTool::Chamfer);
    editor_mut(&mut app, &path).state.dimension_input = "2".into();

    click(&mut app, &path, 5.0, 0.05, None);
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::ChamferFirst { line } if line == along_x
    ));
    click(&mut app, &path, 0.05, 5.0, None);

    let sk = sketch(&app, &path);
    let EntityKind::Line { end: cut_x, .. } = kind_of(sk, along_x) else {
        panic!("still a line");
    };
    let EntityKind::Line { start: cut_y, .. } = kind_of(sk, along_y) else {
        panic!("still a line");
    };
    assert!(near(xy(sk, cut_x), (2.0, 0.0)), "{:?}", xy(sk, cut_x));
    assert!(near(xy(sk, cut_y), (0.0, 2.0)), "{:?}", xy(sk, cut_y));
    let chamfer = sk.entities.iter().any(|e| {
        matches!(e.kind, EntityKind::Line { start, end }
            if (start, end) == (cut_x, cut_y) || (start, end) == (cut_y, cut_x))
    });
    assert!(chamfer, "a Line joins the two cut ends");
    // The corner stays, the two cut points are new; the clicks minted none.
    assert_eq!(points(sk), 5);
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::Idle
    ));
}

#[test]
fn chamfer_refuses_an_unreadable_distance_and_changes_nothing() {
    let (s, _, _) = corner();
    let (mut app, path) = open("chamfer-bad", with_sketch(s), SketchTool::Chamfer);
    let before = sketch(&app, &path).clone();
    editor_mut(&mut app, &path).state.dimension_input = "1,5".into();

    click(&mut app, &path, 5.0, 0.05, None);

    assert_eq!(sketch(&app, &path).entities, before.entities);
    let warnings = &editor(&app, &path).state.solve_warnings;
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("Chamfer:") && w.contains("1,5")),
        "{warnings:?}"
    );
}

#[test]
fn chamfer_longer_than_a_line_is_refused() {
    let (s, along_x, _) = corner();
    let (mut app, path) = open("chamfer-long", with_sketch(s), SketchTool::Chamfer);
    editor_mut(&mut app, &path).state.dimension_input = "20".into();

    click(&mut app, &path, 5.0, 0.05, None);
    click(&mut app, &path, 0.05, 5.0, None);

    let sk = sketch(&app, &path);
    let EntityKind::Line { end, .. } = kind_of(sk, along_x) else {
        panic!("still a line");
    };
    assert!(near(xy(sk, end), (0.0, 0.0)), "the Line keeps its corner");
    assert_eq!(points(sk), 3);
    assert!(
        editor(&app, &path)
            .state
            .solve_warnings
            .iter()
            .any(|w| w.starts_with("Chamfer:"))
    );
}

// ─── Extend (mockup row 187) ────────────────────────────────────────────────

#[test]
fn extend_grows_a_free_end_to_the_line_ahead_and_keeps_it_there() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 0.0);
    let b = s.point(2.0, 0.0);
    let line = s.line(a, b);
    let w0 = s.point(5.0, -1.0);
    let w1 = s.point(5.0, 1.0);
    let wall = s.line(w0, w1);
    let (mut app, path) = open("extend", with_sketch(s), SketchTool::Extend);

    click(&mut app, &path, 1.9, 0.05, None);

    let sk = sketch(&app, &path);
    assert!(near(xy(sk, b), (5.0, 0.0)), "{:?}", xy(sk, b));
    assert!(near(xy(sk, a), (0.0, 0.0)), "the far end stays");
    assert!(matches!(kind_of(sk, line), EntityKind::Line { start, end } if (start, end) == (a, b)));
    assert!(
        sk.constraints.iter().any(|c| matches!(
            c.kind,
            ConstraintKind::PointOnLine { point, line: on } if point == b && on == wall
        )),
        "the end is kept on the wall"
    );
    assert_eq!(points(sk), 4, "no click Point is minted");
}

#[test]
fn extend_with_nothing_ahead_warns_and_changes_nothing() {
    let mut s = Sketch::new();
    let a = s.point(0.0, 0.0);
    let b = s.point(2.0, 0.0);
    s.line(a, b);
    let w0 = s.point(-5.0, -1.0);
    let w1 = s.point(-5.0, 1.0);
    s.line(w0, w1);
    let (mut app, path) = open("extend-none", with_sketch(s), SketchTool::Extend);
    let before = sketch(&app, &path).clone();

    click(&mut app, &path, 1.9, 0.05, None);

    let sk = sketch(&app, &path);
    assert_eq!(sk.entities, before.entities);
    assert!(sk.constraints.is_empty());
    assert!(
        editor(&app, &path)
            .state
            .solve_warnings
            .iter()
            .any(|w| w.starts_with("Extend:"))
    );
}

// ─── Linear Pattern (mockup row 191) ────────────────────────────────────────

#[test]
fn linear_pattern_repeats_the_clicked_source_along_x() {
    let mut s = Sketch::new();
    let source = s.point(1.0, 1.0);
    let (mut app, path) = open("linear", with_sketch(s), SketchTool::LinearPattern);
    editor_mut(&mut app, &path).state.selected_sketch = Some(source);

    click(&mut app, &path, 1.0, 1.0, Some(source));

    let arrays = &sketch(&app, &path).arrays;
    assert_eq!(arrays.len(), 1);
    let ArrayKind::Linear {
        source: from,
        count_expr,
        dx_expr,
        dy_expr,
    } = &arrays[0].kind
    else {
        panic!("a linear array: {:?}", arrays[0].kind);
    };
    assert_eq!(*from, source);
    assert_eq!(count_expr, "4");
    assert_eq!(dx_expr, "5mm");
    assert_eq!(dy_expr, "0mm");
}

#[test]
fn linear_pattern_takes_its_pitch_from_the_dimension_input() {
    let mut s = Sketch::new();
    let source = s.point(1.0, 1.0);
    let (mut app, path) = open("linear-pitch", with_sketch(s), SketchTool::LinearPattern);
    editor_mut(&mut app, &path).state.dimension_input = "2.54".into();

    click(&mut app, &path, 1.0, 1.0, Some(source));

    let arrays = &sketch(&app, &path).arrays;
    assert!(
        matches!(&arrays[0].kind, ArrayKind::Linear { dx_expr, .. } if dx_expr == "2.54mm"),
        "{:?}",
        arrays[0].kind
    );
}

// ─── Mirror about a centerline (mockup row 191) ─────────────────────────────

#[test]
fn mirror_takes_a_centerline_under_the_first_click_as_its_line() {
    let mut s = Sketch::new();
    let c0 = s.point(0.0, -5.0);
    let c1 = s.point(0.0, 5.0);
    let centerline = s.line(c0, c1);
    s.data
        .entities
        .iter_mut()
        .find(|e| e.id == centerline)
        .unwrap()
        .centerline = true;
    let source = s.point(3.0, 1.0);
    let (mut app, path) = open("mirror", with_sketch(s), SketchTool::Mirror);

    click(&mut app, &path, 0.05, 2.0, None);
    assert_eq!(editor(&app, &path).state.selected_sketch, Some(centerline));
    assert_eq!(points(sketch(&app, &path)), 3, "the pick mints no Point");

    click(&mut app, &path, 3.0, 1.0, Some(source));
    let sk = sketch(&app, &path);
    let image = sk.constraints.iter().find_map(|c| match c.kind {
        ConstraintKind::SymmetricAboutLine { p1, p2, line }
            if p1 == source && line == centerline =>
        {
            Some(p2)
        }
        _ => None,
    });
    let image = image.expect("the copy is kept symmetric about the centerline");
    assert!(near(xy(sk, image), (-3.0, 1.0)), "{:?}", xy(sk, image));
}

#[test]
fn mirror_ignores_a_plain_line_under_the_first_click() {
    let mut s = Sketch::new();
    let a = s.point(0.0, -5.0);
    let b = s.point(0.0, 5.0);
    s.line(a, b);
    let (mut app, path) = open("mirror-plain", with_sketch(s), SketchTool::Mirror);

    click(&mut app, &path, 0.05, 2.0, None);

    assert_eq!(editor(&app, &path).state.selected_sketch, None);
    assert!(
        editor(&app, &path)
            .state
            .solve_warnings
            .iter()
            .any(|w| w.starts_with("Mirror:"))
    );
}

// ─── Pads and the sketch (mockup row 192) ───────────────────────────────────

/// A footprint with pad "6" driven by a centre Point inside a 2 × 1 mm
/// rectangle of Lines. Returns it with the centre and the four Lines.
fn pad_in_a_profile() -> (Footprint, SketchEntityId, Vec<SketchEntityId>) {
    let mut s = Sketch::new();
    let corners = [
        s.point(-1.0, -0.5),
        s.point(1.0, -0.5),
        s.point(1.0, 0.5),
        s.point(-1.0, 0.5),
    ];
    let lines = (0..4)
        .map(|i| s.line(corners[i], corners[(i + 1) % 4]))
        .collect::<Vec<_>>();
    let centre = s.point(0.0, 0.0);
    s.data
        .entities
        .iter_mut()
        .find(|e| e.id == centre)
        .unwrap()
        .pad = Some(PadAttr {
        number: "6".into(),
        ..PadAttr::default()
    });
    let mut fp = with_sketch(s);
    fp.pads.push(Pad {
        number: "6".into(),
        ..Pad::default()
    });
    (fp, centre, lines)
}

fn open_pad(stem: &str) -> (Signex, PathBuf, SketchEntityId, Vec<SketchEntityId>) {
    let (fp, centre, lines) = pad_in_a_profile();
    let (mut app, path) = open(stem, fp, SketchTool::Select);
    let editor = editor_mut(&mut app, &path);
    editor.state.mode = EditorMode::Normal;
    editor.state.pads[0].sketch_entity_id = Some(centre);
    (app, path, centre, lines)
}

fn edit_in_sketch(app: &mut Signex) {
    drop(app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorEditPadInSketch { pad_idx: 0 },
    ))));
}

#[test]
fn edit_in_sketch_focuses_the_pad_and_offers_back_to_pad() {
    let (mut app, path, centre, _) = open_pad("link");

    edit_in_sketch(&mut app);

    let state = &editor(&app, &path).state;
    assert_eq!(state.mode, EditorMode::Sketch);
    assert_eq!(state.selected_sketch, Some(centre));
    assert_eq!(
        state.live_pad_link().map(|f| (f.pad_idx, f.entity)),
        Some((0, centre))
    );
    assert_eq!(
        state.pad_link_offer(),
        Some(PadLinkOffer::BackToPad {
            pad_idx: 0,
            entity: centre
        })
    );
}

#[test]
fn the_driving_profile_is_the_pad_entity_and_the_loop_around_it() {
    let (fp, centre, lines) = pad_in_a_profile();
    let profile = driving_profile(fp.sketch.as_ref().unwrap(), centre, None);
    assert_eq!(profile[0], centre);
    assert_eq!(profile.len(), 5, "{profile:?}");
    assert!(lines.iter().all(|l| profile.contains(l)));
}

#[test]
fn back_to_pad_returns_to_footprint_mode_with_the_pad_selected() {
    let (mut app, path, centre, _) = open_pad("back");
    edit_in_sketch(&mut app);
    let dirty_before = editor(&app, &path).is_dirty();

    edit(
        &mut app,
        &path,
        FootprintEditorMsg::PadLink(PadLinkMsg::PadLinkPressed { entity: centre }),
    );

    let state = &editor(&app, &path).state;
    assert_eq!(state.mode, EditorMode::Normal);
    assert_eq!(state.selected_pad, Some(0));
    assert_eq!(state.selected_sketch, None);
    assert!(state.pad_link.is_none());
    assert_eq!(
        editor(&app, &path).is_dirty(),
        dirty_before,
        "the link edits nothing"
    );
}

#[test]
fn the_focus_ends_on_a_mode_change() {
    let (mut app, path, _, _) = open_pad("mode");
    edit_in_sketch(&mut app);

    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SetMode(EditorMode::Normal),
    );

    let state = &editor(&app, &path).state;
    assert!(state.pad_link.is_none());
    assert!(state.live_pad_link().is_none());
    assert_eq!(state.pad_link_offer(), None);
}

#[test]
fn the_focus_ends_when_the_selection_moves_and_a_pad_entity_offers_jump_to_pad() {
    let (mut app, path, centre, lines) = open_pad("select");
    edit_in_sketch(&mut app);

    editor_mut(&mut app, &path).state.selected_sketch = Some(lines[0]);
    let state = &editor(&app, &path).state;
    assert!(
        state.live_pad_link().is_none(),
        "the focus followed the selection away"
    );
    assert_eq!(state.pad_link_offer(), None, "a plain Line drives no pad");

    editor_mut(&mut app, &path).state.selected_sketch = Some(centre);
    editor_mut(&mut app, &path).state.pad_link = None;
    assert_eq!(
        editor(&app, &path).state.pad_link_offer(),
        Some(PadLinkOffer::JumpToPad { entity: centre })
    );
}

/// A value edit open on the drawing, a menu and a selected pattern: what a
/// mode change must not carry into Footprint mode.
fn open_overlays(app: &mut Signex, path: &PathBuf) {
    use signex_app::library::editor::footprint::{
        CanvasMenu, MenuTarget, PatternField, ValueEdit, ValueTarget,
    };
    use signex_sketch::array::ArrayId;
    let array = ArrayId::new();
    let overlay = &mut editor_mut(app, path).state.sketch_overlay;
    overlay.value_edit = Some(ValueEdit {
        target: ValueTarget::Pattern(array, PatternField::Count),
        buffer: "4".into(),
        invalid: false,
    });
    overlay.menu = Some(CanvasMenu {
        target: MenuTarget::Pattern(array),
        x: 10.0,
        y: 10.0,
    });
    overlay.selected_pattern = Some(array);
}

#[test]
fn leaving_sketch_mode_closes_the_canvas_overlays() {
    use signex_app::library::editor::footprint::SketchOverlay;
    let (mut app, path, centre, _) = open_pad("overlay");

    edit_in_sketch(&mut app);
    open_overlays(&mut app, &path);
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SetMode(EditorMode::Normal),
    );
    assert_eq!(
        editor(&app, &path).state.sketch_overlay,
        SketchOverlay::default(),
        "the mode switcher"
    );

    edit_in_sketch(&mut app);
    open_overlays(&mut app, &path);
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::PadLink(PadLinkMsg::PadLinkPressed { entity: centre }),
    );
    let state = &editor(&app, &path).state;
    assert_eq!(state.mode, EditorMode::Normal);
    assert_eq!(state.selected_pad, Some(0), "the pad is selected to act on");
    assert_eq!(
        state.sketch_overlay,
        SketchOverlay::default(),
        "Back to pad"
    );

    open_overlays(&mut app, &path);
    edit_in_sketch(&mut app);
    assert_eq!(
        editor(&app, &path).state.sketch_overlay,
        SketchOverlay::default(),
        "Edit in Sketch starts clean"
    );
}
