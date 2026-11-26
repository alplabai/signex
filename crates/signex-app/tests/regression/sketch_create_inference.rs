//! Caner (2026-11-20), items 1 and 6 (mockup row 187, mockup row 179, mockup row 188): Sketch Create ▾
//! offers Slot and Polygon after Point on both hosts, each placed as one
//! undo step; the Midpoint, Centre and Intersection snaps land their
//! constraint, and Ctrl places without inference, the Line's auto H / V
//! included.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::BoardBackdrop;
use signex_app::library::editor::footprint::sketch_mode::menus::create_entries;
use signex_app::library::editor::footprint::snap::{
    SketchInference, SnapKind, SnapResult, click_inference, snap_cursor_with,
};
use signex_app::library::editor::footprint::{
    EditorMode, FootprintEditorState, SketchTool, ToolPending,
};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_i18n::{I18n, LanguagePref};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::constraint::ConstraintKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// A sketch under construction: one plane, Points, Lines and Circles.
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

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(id, self.plane, kind));
        id
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        self.add(EntityKind::Point { x, y })
    }

    fn line(&mut self, from: (f64, f64), to: (f64, f64)) -> SketchEntityId {
        let start = self.point(from.0, from.1);
        let end = self.point(to.0, to.1);
        self.add(EntityKind::Line { start, end })
    }
}

/// `sketch` in a footprint tab, in sketch mode with `tool` armed.
fn open(stem: &str, sketch: Sketch, tool: SketchTool) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(sketch.data);
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
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

fn click(app: &mut Signex, path: &Path, at: (f64, f64)) {
    let msg = FootprintEditorMsg::SketchToolClick {
        x_mm: at.0,
        y_mm: at.1,
        snap_id: None,
    };
    edit(app, path, msg);
}

fn click_inferred(app: &mut Signex, path: &Path, at: (f64, f64), inference: SketchInference) {
    let msg = FootprintEditorMsg::SketchToolClickInferred {
        x_mm: at.0,
        y_mm: at.1,
        snap_id: None,
        inference,
    };
    edit(app, path, msg);
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

fn sketch<'a>(app: &'a Signex, path: &PathBuf) -> &'a SketchData {
    editor(app, path).primitive().sketch.as_ref().unwrap()
}

fn undo_steps(app: &Signex, path: &PathBuf) -> usize {
    editor(app, path).history.undo_len()
}

fn constraints(app: &Signex, path: &PathBuf) -> Vec<ConstraintKind> {
    sketch(app, path)
        .constraints
        .iter()
        .map(|c| c.kind.clone())
        .collect()
}

/// The Point the Line tool's pending chain starts from.
fn line_anchor(app: &Signex, path: &PathBuf) -> SketchEntityId {
    match editor(app, path).state.tool_pending {
        ToolPending::LineFirst { first } => first,
        ref other => panic!("not a pending Line: {other:?}"),
    }
}

fn count(app: &Signex, path: &PathBuf, keep: impl Fn(&Entity) -> bool) -> usize {
    sketch(app, path)
        .entities
        .iter()
        .filter(|e| keep(e))
        .count()
}

fn is_outline_line(e: &Entity) -> bool {
    matches!(e.kind, EntityKind::Line { .. }) && !e.construction
}

fn xy(sketch: &SketchData, id: SketchEntityId) -> (f64, f64) {
    sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Point { x, y } if e.id == id => Some((x, y)),
            _ => None,
        })
        .expect("a point")
}

fn create_labels(state: &FootprintEditorState) -> Vec<String> {
    let i18n = I18n::new(&LanguagePref::Locale("en-US".into()));
    create_entries(state, PathBuf::from("t"), ThemeId::DEFAULT, &i18n)
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => Some(item.label.replace(['\u{2068}', '\u{2069}'], "")),
            _ => None,
        })
        .collect()
}

#[test]
fn create_offers_slot_and_polygon_after_point_on_the_footprint_and_the_board() {
    let footprint = FootprintEditorState::from_footprint(&Footprint::empty("t"));
    let mut board = FootprintEditorState::from_footprint(&Footprint::empty("t"));
    let pcb = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.board_backdrop = Some(Arc::new(BoardBackdrop::from_board(&pcb)));
    for state in [&footprint, &board] {
        let rows = create_labels(state);
        let point = rows.iter().position(|r| r == "Point").expect("Point");
        assert_eq!(rows[point + 1..], ["Slot", "Polygon"]);
    }
}

#[test]
fn a_slot_is_two_arcs_two_lines_and_two_dimensions_in_one_undo_step_and_bakes() {
    let (mut app, path) = open("slot", Sketch::new(), SketchTool::Slot);
    click(&mut app, &path, (0.0, 0.0));
    click(&mut app, &path, (10.0, 0.0));
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::SlotSecond { .. }
    ));
    let before = undo_steps(&app, &path);
    click(&mut app, &path, (5.0, 2.0));
    assert_eq!(undo_steps(&app, &path), before + 1, "the slot is one step");

    let arcs = count(&app, &path, |e| matches!(e.kind, EntityKind::Arc { .. }));
    assert_eq!(arcs, 2);
    assert_eq!(count(&app, &path, is_outline_line), 2);
    let dimensions = constraints(&app, &path)
        .iter()
        .filter(|c| c.dim_target().is_some())
        .count();
    assert_eq!(dimensions, 2, "centre distance and width");
    let ed = editor(&app, &path);
    assert_eq!(ed.state.conflict.rejections(), 0, "the slot solves");
    assert!(matches!(ed.state.tool_pending, ToolPending::Idle));

    let side = sketch(&app, &path)
        .entities
        .iter()
        .find(|e| is_outline_line(e))
        .map(|e| e.id)
        .expect("a side");
    let profile = signex_bake::profile::trace_closed_profile_entities(sketch(&app, &path), side)
        .expect("the slot is a closed profile");
    assert_eq!(profile.edges.len(), 4);
}

#[test]
fn a_typed_dimension_is_the_slot_centre_distance_and_esc_cancels() {
    let (mut app, path) = open("slot-len", Sketch::new(), SketchTool::Slot);
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchDimensionInput("8".into()),
    );
    click(&mut app, &path, (0.0, 0.0));
    click(&mut app, &path, (3.0, 0.0));
    let ToolPending::SlotSecond { a, b } = editor(&app, &path).state.tool_pending else {
        panic!("both centres placed");
    };
    let (pa, pb) = (xy(sketch(&app, &path), a), xy(sketch(&app, &path), b));
    assert!(((pb.0 - pa.0).hypot(pb.1 - pa.1) - 8.0).abs() < 1e-9);

    edit(&mut app, &path, FootprintEditorMsg::SketchToolEscape);
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::Idle
    ));
}

#[test]
fn a_polygon_with_five_sides_is_five_equal_lines_and_a_bad_count_refuses() {
    let (mut app, path) = open("polygon", Sketch::new(), SketchTool::Polygon);
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchDimensionInput("5".into()),
    );
    click(&mut app, &path, (0.0, 0.0));
    let before = undo_steps(&app, &path);
    click(&mut app, &path, (4.0, 0.0));
    assert_eq!(
        undo_steps(&app, &path),
        before + 1,
        "the polygon is one step"
    );
    let data = sketch(&app, &path);
    let lengths: Vec<f64> = data
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Line { start, end } if !e.construction => {
                let (s, f) = (xy(data, start), xy(data, end));
                Some((f.0 - s.0).hypot(f.1 - s.1))
            }
            _ => None,
        })
        .collect();
    assert_eq!(lengths.len(), 5);
    assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-6));
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::Idle
    ));

    let (mut app, path) = open("polygon-bad", Sketch::new(), SketchTool::Polygon);
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchDimensionInput("2".into()),
    );
    click(&mut app, &path, (0.0, 0.0));
    let entities = sketch(&app, &path).entities.len();
    click(&mut app, &path, (4.0, 0.0));
    assert_eq!(
        sketch(&app, &path).entities.len(),
        entities,
        "nothing placed"
    );
    let ed = editor(&app, &path);
    assert!(
        ed.state
            .solve_warnings
            .iter()
            .any(|w| w.contains("Polygon")),
        "{:?}",
        ed.state.solve_warnings
    );
    assert!(matches!(
        ed.state.tool_pending,
        ToolPending::PolygonCentre { .. }
    ));
}

#[test]
fn a_midpoint_snap_lands_a_midpoint_constraint() {
    let mut s = Sketch::new();
    let line = s.line((0.0, 0.0), (10.0, 0.0));
    let (mut app, path) = open("midpoint", s, SketchTool::Line);
    click_inferred(&mut app, &path, (5.0, 0.0), SketchInference::Midpoint(line));
    let point = line_anchor(&app, &path);
    assert!(constraints(&app, &path).contains(&ConstraintKind::Midpoint { point, line }));

    let mut s = Sketch::new();
    let line = s.line((0.0, 0.0), (10.0, 0.0));
    let (mut app, path) = open("midpoint-point", s, SketchTool::Point);
    let msg = FootprintEditorMsg::SketchPlacePointInferred {
        x_mm: 5.0,
        y_mm: 0.0,
        inference: SketchInference::Midpoint(line),
    };
    edit(&mut app, &path, msg);
    let midpoints = constraints(&app, &path)
        .iter()
        .filter(|c| matches!(c, ConstraintKind::Midpoint { line: l, .. } if *l == line))
        .count();
    assert_eq!(midpoints, 1, "the Point tool lands it too");
}

#[test]
fn a_centre_snap_lands_a_coincident_on_the_centre() {
    let mut s = Sketch::new();
    let centre = s.point(20.0, 0.0);
    let circle = s.add(EntityKind::Circle {
        center: centre,
        radius: 2.0,
    });
    let (mut app, path) = open("centre", s, SketchTool::Line);
    click_inferred(
        &mut app,
        &path,
        (20.0, 0.0),
        SketchInference::Centre(circle),
    );
    let point = line_anchor(&app, &path);
    assert_ne!(point, centre, "the click places its own point");
    assert!(
        constraints(&app, &path).contains(&ConstraintKind::Coincident {
            p1: point,
            p2: centre,
        })
    );
}

#[test]
fn an_intersection_snap_lands_a_point_on_each_line() {
    let mut s = Sketch::new();
    let rising = s.line((0.0, 0.0), (10.0, 10.0));
    let flat = s.line((0.0, 4.0), (12.0, 4.0));
    let (mut app, path) = open("crossing", s, SketchTool::Line);
    click_inferred(
        &mut app,
        &path,
        (4.0, 4.0),
        SketchInference::Intersection(rising, flat),
    );
    let point = line_anchor(&app, &path);
    let all = constraints(&app, &path);
    for line in [rising, flat] {
        assert!(all.contains(&ConstraintKind::PointOnLine { point, line }));
    }
}

#[test]
fn ctrl_lands_no_inference_and_a_near_horizontal_line_gets_no_h() {
    let (mut app, path) = open("ctrl", Sketch::new(), SketchTool::Line);
    click_inferred(&mut app, &path, (0.0, 0.0), SketchInference::Off);
    click_inferred(&mut app, &path, (10.0, 0.3), SketchInference::Off);
    assert_eq!(count(&app, &path, is_outline_line), 1);
    assert!(
        constraints(&app, &path).is_empty(),
        "no inferred constraint"
    );

    let (mut app, path) = open("no-ctrl", Sketch::new(), SketchTool::Line);
    click(&mut app, &path, (0.0, 0.0));
    click(&mut app, &path, (10.0, 0.3));
    let horizontal = constraints(&app, &path)
        .iter()
        .any(|c| matches!(c, ConstraintKind::Horizontal { .. }));
    assert!(horizontal, "within ±5° the line still gets H");
}

/// An editor in sketch mode with the Line tool armed and intersection
/// snaps on.
fn drawing() -> FootprintEditorState {
    let mut state = FootprintEditorState::empty();
    state.mode = EditorMode::Sketch;
    state.active_tool = SketchTool::Line;
    state.snap_options.snap_intersections = true;
    state
}

#[test]
fn midpoint_and_centre_snap_before_the_axis_and_ctrl_skips_them() {
    let mut s = Sketch::new();
    let line = s.line((0.0, 0.0), (10.0, 0.0));
    let centre = s.point(20.0, 0.0);
    let circle = s.add(EntityKind::Circle {
        center: centre,
        radius: 2.0,
    });
    let anchor = s.point(0.0, -5.0);
    let mut state = drawing();
    let data = Some(&s.data);

    let mid = snap_cursor_with((5.02, 0.02), data, &state, None, true);
    assert_eq!(mid.kind, SnapKind::Midpoint(line));
    assert_eq!(mid.pos, (5.0, 0.0));
    let at_centre = snap_cursor_with((20.02, 0.01), data, &state, None, true);
    assert_eq!(at_centre.kind, SnapKind::Centre(circle));
    let on_point = snap_cursor_with((20.0, 0.0), data, &state, Some(centre), true);
    assert_eq!(
        on_point.kind,
        SnapKind::Centre(circle),
        "a centre point reads as Centre"
    );

    // Away from any midpoint, 2° off horizontal from the anchor: H.
    state.tool_pending = ToolPending::LineFirst { first: anchor };
    let near_axis = snap_cursor_with((8.0, -4.72), data, &state, None, true);
    assert_eq!(near_axis.kind, SnapKind::Horizontal);

    // Ctrl: no Midpoint, Centre or H; point reuse and grid stay.
    let off = |raw, hit| snap_cursor_with(raw, data, &state, hit, false).kind;
    assert_eq!(off((5.02, 0.02), None), SnapKind::Grid);
    assert_eq!(off((8.0, -4.72), None), SnapKind::Grid);
    assert_eq!(off((20.0, 0.0), Some(centre)), SnapKind::Point(centre));

    // Snap ▾ turns the midpoint snap off.
    state.tool_pending = ToolPending::Idle;
    state.snap_options.snap_midpoints = false;
    let unswitched = snap_cursor_with((5.02, 0.02), data, &state, None, true);
    assert_ne!(unswitched.kind, SnapKind::Midpoint(line));
}

#[test]
fn a_click_infers_from_the_snap_it_took_or_nothing_under_ctrl() {
    let mut s = Sketch::new();
    let rising = s.line((0.0, 0.0), (10.0, 10.0));
    let flat = s.line((0.0, 4.0), (12.0, 4.0));
    let state = drawing();
    let data = Some(&s.data);
    let snap = |pos, kind| Some(SnapResult { pos, kind });

    let midpoint = snap((5.0, 5.0), SnapKind::Midpoint(rising));
    assert_eq!(
        click_inference(midpoint, (5.0, 5.0), data, &state, false),
        Some(SketchInference::Midpoint(rising))
    );
    assert_eq!(
        click_inference(midpoint, (5.0, 5.0), data, &state, true),
        Some(SketchInference::Off),
        "Ctrl"
    );
    assert_eq!(
        click_inference(midpoint, (7.0, 5.0), data, &state, false),
        None,
        "the snap was elsewhere"
    );
    let crossing = snap((4.0, 4.0), SnapKind::Intersection);
    let Some(SketchInference::Intersection(a, b)) =
        click_inference(crossing, (4.0, 4.0), data, &state, false)
    else {
        panic!("an intersection");
    };
    assert!([a, b].contains(&rising) && [a, b].contains(&flat));
    let grid = snap((1.0, 1.0), SnapKind::Grid);
    assert_eq!(click_inference(grid, (1.0, 1.0), data, &state, false), None);

    let mut trim = state.clone();
    trim.active_tool = SketchTool::Trim;
    assert_eq!(
        click_inference(midpoint, (5.0, 5.0), data, &trim, true),
        None,
        "only a drawing tool infers"
    );
}
