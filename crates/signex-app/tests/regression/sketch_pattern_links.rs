//! #191 (decision 10, Caner 2026-11-20) — Mirror with a pattern selected
//! keeps a live mirror of it; a pattern tool with a pattern selected makes
//! a pattern of that pattern; a link that would loop is refused with a
//! warning and leaves no undo step.

use std::path::PathBuf;

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::sketch_dispatch::apply_sketch_edit;
use signex_app::library::editor::footprint::sketch_mode::SketchEdit;
use signex_app::library::editor::footprint::sketch_mode::value_edit::SketchCanvasMsg;
use signex_app::library::editor::footprint::state::{EditorMode, SketchTool};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::{Footprint, FootprintFile, Pad};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::PatternSource;

/// A pad source at (1, 0), a centre at the origin and a vertical Line at
/// x = 5, with one array of the source.
struct Fixture {
    sketch: SketchData,
    source: SketchEntityId,
    centre: SketchEntityId,
    line: SketchEntityId,
}

fn fixture() -> Fixture {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let point = |sketch: &mut SketchData, x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let source = point(&mut sketch, 1.0, 0.0);
    sketch.entities.last_mut().unwrap().pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    let centre = point(&mut sketch, 0.0, 0.0);
    let a = point(&mut sketch, 5.0, -1.0);
    let b = point(&mut sketch, 5.0, 1.0);
    let line = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        line,
        plane,
        EntityKind::Line { start: a, end: b },
    ));
    Fixture {
        sketch,
        source,
        centre,
        line,
    }
}

fn linear(source: SketchEntityId, count: u32, dx_mm: f64) -> Array {
    Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source,
            count_expr: count.to_string(),
            dx_expr: format!("{dx_mm}mm"),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::default(),
    }
}

fn polar(source: SketchEntityId, centre: SketchEntityId) -> Array {
    Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source,
            center: centre,
            count_expr: "4".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    }
}

/// `sketch` open in a footprint tab in sketch mode with `tool` armed,
/// solved and baked once.
fn open(stem: &str, sketch: SketchData, tool: SketchTool) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(sketch);
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
    rebake(&mut app, &path);
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

fn pads<'a>(app: &'a Signex, path: &PathBuf) -> &'a [Pad] {
    &editor(app, path).primitive().pads
}

fn rebake(app: &mut Signex, path: &PathBuf) {
    editor_mut(app, path)
        .with_parts(|state, fp| apply_sketch_edit(state, fp, SketchEdit::ForceRebuild))
        .expect("the fixture solves");
}

fn send(app: &mut Signex, path: &PathBuf, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

fn select_pattern(app: &mut Signex, path: &PathBuf, array: ArrayId) {
    send(
        app,
        path,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::PatternSelected(Some(array))),
    );
}

fn click(app: &mut Signex, path: &PathBuf, x_mm: f64, y_mm: f64, snap: Option<SketchEntityId>) {
    send(
        app,
        path,
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: snap,
        },
    );
}

fn has_pad_at(pads: &[Pad], x: f64, y: f64) -> bool {
    pads.iter()
        .any(|p| (p.position[0] - x).abs() < 1e-6 && (p.position[1] - y).abs() < 1e-6)
}

#[test]
fn mirror_with_a_pattern_selected_adds_a_live_mirror_that_follows_it() {
    let mut f = fixture();
    let row = linear(f.source, 2, 1.0);
    let row_id = row.id;
    f.sketch.arrays.push(row);
    let entities = f.sketch.entities.len();
    let (mut app, path) = open("mirror-pattern", f.sketch, SketchTool::Mirror);
    assert_eq!(pads(&app, &path).len(), 2);
    select_pattern(&mut app, &path, row_id);
    let undo_before = editor(&app, &path).history.undo_len();

    click(&mut app, &path, 5.0, 0.0, None);

    let sk = sketch(&app, &path);
    assert_eq!(sk.pattern_mirrors.len(), 1, "a live mirror, not copies");
    assert_eq!(sk.pattern_mirrors[0].pattern, row_id);
    assert_eq!(sk.pattern_mirrors[0].line, f.line);
    assert_eq!(
        sk.entities.len(),
        entities,
        "no geometry is copied or minted"
    );
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before + 1);
    assert!(editor(&app, &path).is_dirty());
    let baked = pads(&app, &path);
    assert_eq!(baked.len(), 4, "{baked:?}");
    assert!(has_pad_at(baked, 9.0, 0.0) && has_pad_at(baked, 8.0, 0.0));

    // The row grows to three: the mirror follows on the next bake.
    if let Some(ArrayKind::Linear { count_expr, .. }) = editor_mut(&mut app, &path)
        .primitive_mut()
        .sketch
        .as_mut()
        .and_then(|s| s.arrays.iter_mut().find(|a| a.id == row_id))
        .map(|a| &mut a.kind)
    {
        *count_expr = "3".into();
    }
    rebake(&mut app, &path);
    let baked = pads(&app, &path);
    assert_eq!(baked.len(), 6, "{baked:?}");
    assert!(has_pad_at(baked, 7.0, 0.0), "the image of (3, 0)");
}

#[test]
fn a_pattern_tool_on_a_selected_pattern_bakes_every_instance_of_it() {
    let mut f = fixture();
    let ring = polar(f.source, f.centre);
    let ring_id = ring.id;
    f.sketch.arrays.push(ring);
    let (mut app, path) = open("pattern-of-pattern", f.sketch, SketchTool::LinearPattern);
    assert_eq!(pads(&app, &path).len(), 4);
    select_pattern(&mut app, &path, ring_id);
    editor_mut(&mut app, &path).state.dimension_input = "10".into();

    click(&mut app, &path, 1.0, 0.0, Some(f.source));

    let sk = sketch(&app, &path);
    assert_eq!(sk.arrays.len(), 2);
    let outer = sk.arrays.iter().find(|a| a.id != ring_id).unwrap();
    assert_eq!(sk.pattern_source_of(outer.id), Some(ring_id));
    assert_eq!(outer.kind.source(), f.source, "the innermost source entity");
    let baked = pads(&app, &path);
    // Four steps of the four-pad ring.
    assert_eq!(baked.len(), 16, "{baked:?}");
    for step in 0..4 {
        let dx = 10.0 * f64::from(step);
        for (x, y) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
            assert!(has_pad_at(baked, x + dx, y), "({}, {y})", x + dx);
        }
    }
    let mut numbers: Vec<&str> = baked.iter().map(|p| p.number.as_str()).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), 16, "every pad has its own number");
}

#[test]
fn a_link_onto_a_looping_chain_is_refused_with_a_warning_and_no_undo_step() {
    let mut f = fixture();
    let ring = polar(f.source, f.centre);
    let ring_id = ring.id;
    let other = linear(f.source, 2, 3.0);
    let other_id = other.id;
    f.sketch.arrays.extend([ring, other]);
    // A damaged file: the two arrays repeat each other.
    f.sketch.pattern_sources = vec![
        PatternSource {
            array: other_id,
            source: ring_id,
        },
        PatternSource {
            array: ring_id,
            source: other_id,
        },
    ];
    let (mut app, path) = open("pattern-loop", f.sketch, SketchTool::LinearPattern);
    assert!(
        editor(&app, &path)
            .state
            .solve_warnings
            .iter()
            .any(|w| w.contains("loops")),
        "the bake refuses the loop: {:?}",
        editor(&app, &path).state.solve_warnings
    );
    select_pattern(&mut app, &path, ring_id);
    let before = sketch(&app, &path).clone();
    let undo_before = editor(&app, &path).history.undo_len();
    let dirty_before = editor(&app, &path).is_dirty();

    click(&mut app, &path, 1.0, 0.0, Some(f.source));

    assert_eq!(sketch(&app, &path), &before, "nothing is added");
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before);
    assert_eq!(editor(&app, &path).is_dirty(), dirty_before);
    let warnings = &editor(&app, &path).state.solve_warnings;
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("Pattern:") && w.contains("repeat itself")),
        "{warnings:?}"
    );
}
