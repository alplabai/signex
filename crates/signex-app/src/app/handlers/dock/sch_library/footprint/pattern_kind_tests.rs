//! #2596: the Pattern form's kind segment switches a pattern (DECISIONS
//! 2026-11-27, decisions 1–3). Linear and Grid switch in one undo step;
//! Polar waits for a centre pick, the click switches it in one undo step,
//! and Esc leaves the pattern as it was.

use std::path::{Path, PathBuf};

use signex_bake::array::PatternKind;
use signex_library::{Footprint, FootprintFile};
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::attr::PadAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::editor::footprint::ToolPending;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// An app on a footprint whose sketch repeats pad 1 four times along X,
/// with instance 2 skipped.
pub(super) fn app_on_a_linear(path: &Path) -> (Signex, ArrayId) {
    let path = path.to_path_buf();
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    sketch.entities.push(point);
    let array = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source,
            count_expr: "4".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: vec![(2, 0)],
            }),
        },
        numbering: NumberingScheme::default(),
    };
    let array_id = array.id;
    sketch.arrays.push(array);
    let mut footprint = Footprint::empty("linear");
    footprint.sketch = Some(sketch);
    let file = FootprintFile::from_footprint(footprint);
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "linear".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, array_id)
}

pub(super) fn editor<'a>(app: &'a Signex, path: &Path) -> &'a FootprintDocument {
    &app.document_state.footprint_editors[path]
}

pub(super) fn sketch<'a>(app: &'a Signex, path: &Path) -> &'a SketchData {
    editor(app, path)
        .primitive()
        .sketch
        .as_ref()
        .expect("a sketch")
}

pub(super) fn kind<'a>(app: &'a Signex, path: &Path, array_id: ArrayId) -> &'a ArrayKind {
    let array = sketch(app, path).arrays.iter().find(|a| a.id == array_id);
    &array.expect("the pattern").kind
}

pub(super) fn is_idle(app: &Signex, path: &Path) -> bool {
    matches!(editor(app, path).state.tool_pending, ToolPending::Idle)
}

pub(super) fn send(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

#[test]
fn linear_to_grid_and_back_are_one_undo_step_each() {
    let path = PathBuf::from("kind-linear-grid.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let start = kind(&app, &path, array_id).clone();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Grid);
    let ArrayKind::Grid {
        nx_expr,
        ny_expr,
        depopulation,
        ..
    } = kind(&app, &path, array_id)
    else {
        panic!("a grid");
    };
    assert_eq!((nx_expr.as_str(), ny_expr.as_str()), ("4", "1"));
    assert_eq!(
        depopulation.as_ref().unwrap().suppressed_instances,
        vec![(2, 0)]
    );
    assert_eq!(editor(&app, &path).history.undo_len(), 1);
    assert!(editor(&app, &path).is_dirty());

    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Linear);
    assert_eq!(kind(&app, &path, array_id), &start);
    assert_eq!(editor(&app, &path).history.undo_len(), 2);

    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert!(matches!(
        kind(&app, &path, array_id),
        ArrayKind::Grid { .. }
    ));
    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert_eq!(kind(&app, &path, array_id), &start);
}

/// Decision 2: Polar first asks for a centre; the click on the canvas is the
/// centre, and the switch with its centre Point is one undo step.
#[test]
fn polar_waits_for_a_centre_and_the_click_switches_it() {
    let path = PathBuf::from("kind-polar-pick.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let start = kind(&app, &path, array_id).clone();
    let entities = sketch(&app, &path).entities.len();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    assert!(matches!(
        editor(&app, &path).state.tool_pending,
        ToolPending::RepickPolarCenter {
            array_id: id,
            is_kind_switch: true,
        } if id == array_id
    ));
    assert_eq!(kind(&app, &path, array_id), &start, "nothing switches yet");
    assert_eq!(editor(&app, &path).history.undo_len(), 0);

    send(
        &mut app,
        &path,
        FootprintEditorMsg::SketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id: None,
        },
    );
    let ArrayKind::Polar {
        center,
        count_expr,
        sweep_angle_expr,
        depopulation,
        ..
    } = kind(&app, &path, array_id).clone()
    else {
        panic!("a polar pattern");
    };
    let centre = sketch(&app, &path).entities.iter().find(|e| e.id == center);
    assert!(
        matches!(centre.map(|e| &e.kind), Some(EntityKind::Point { x, y }) if (x - 3.0).abs() < 1e-9 && (y - 4.0).abs() < 1e-9),
        "the clicked Point is the centre"
    );
    assert_eq!(
        (count_expr.as_str(), sweep_angle_expr.as_str()),
        ("4", "360deg")
    );
    assert_eq!(depopulation.unwrap().suppressed_instances, vec![(2, 0)]);
    assert!(is_idle(&app, &path));
    assert_eq!(editor(&app, &path).history.undo_len(), 1);

    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert_eq!(kind(&app, &path, array_id), &start);
    assert_eq!(
        sketch(&app, &path).entities.len(),
        entities,
        "the centre goes too"
    );
}

/// Decision 2: Esc cancels the switch and the pattern stays as it was.
#[test]
fn esc_cancels_the_switch_to_polar() {
    let path = PathBuf::from("kind-polar-esc.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let start = kind(&app, &path, array_id).clone();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    send(&mut app, &path, FootprintEditorMsg::ToolEscape);
    assert!(is_idle(&app, &path));
    assert_eq!(kind(&app, &path, array_id), &start);
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
    assert!(!editor(&app, &path).is_dirty());

    // A click after the cancel switches nothing.
    send(
        &mut app,
        &path,
        FootprintEditorMsg::SketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id: None,
        },
    );
    assert!(matches!(
        kind(&app, &path, array_id),
        ArrayKind::Linear { .. }
    ));
}

/// The pattern's own kind segment cancels a centre pick still waiting.
#[test]
fn the_current_kind_cancels_a_waiting_centre_pick() {
    let path = PathBuf::from("kind-polar-back.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let start = kind(&app, &path, array_id).clone();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Linear);
    assert!(is_idle(&app, &path));
    assert_eq!(kind(&app, &path, array_id), &start);
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
}

pub(super) fn click(app: &mut Signex, path: &Path, snap_id: Option<SketchEntityId>) {
    send(
        app,
        path,
        FootprintEditorMsg::SketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id,
        },
    );
}

pub(super) fn polar_centre(app: &Signex, path: &Path, array_id: ArrayId) -> SketchEntityId {
    match kind(app, path, array_id) {
        ArrayKind::Polar { center, .. } => *center,
        other => panic!("a polar pattern, not {other:?}"),
    }
}

pub(super) fn has_entity(app: &Signex, path: &Path, id: SketchEntityId) -> bool {
    sketch(app, path).entities.iter().any(|e| e.id == id)
}

pub(super) fn sketch_mut<'a>(app: &'a mut Signex, path: &Path) -> &'a mut SketchData {
    let doc = app.document_state.footprint_editors.get_mut(path).unwrap();
    doc.primitive_mut().sketch.as_mut().expect("a sketch")
}

/// The centre the switch to Polar minted goes again when the pattern
/// leaves Polar, in the same undo step.
#[test]
fn a_minted_centre_goes_when_the_pattern_leaves_polar() {
    let path = PathBuf::from("kind-minted-centre.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let entities = sketch(&app, &path).entities.len();
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    click(&mut app, &path, None);
    let centre = polar_centre(&app, &path, array_id);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Grid);
    assert!(!has_entity(&app, &path, centre));
    assert_eq!(sketch(&app, &path).entities.len(), entities);
    assert_eq!(editor(&app, &path).history.undo_len(), 2);
    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert_eq!(polar_centre(&app, &path, array_id), centre);
    assert!(
        has_entity(&app, &path, centre),
        "undo brings the centre back"
    );
    // #3232: the undo brings back that the switch minted it, too.
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Linear);
    assert!(
        !has_entity(&app, &path, centre),
        "the centre goes after the undo"
    );
    assert_eq!(sketch(&app, &path).entities.len(), entities);
    let doc = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(doc.undo());
    assert!(doc.redo());
    assert!(!has_entity(&app, &path, centre), "redo removes it again");
}

/// A centre the user drew and then picked stays.
#[test]
fn a_drawn_centre_stays_when_the_pattern_leaves_polar() {
    let path = PathBuf::from("kind-drawn-centre.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    let drawn = SketchEntityId::new();
    let sketch = sketch_mut(&mut app, &path);
    let plane = sketch.planes[0].id;
    sketch.entities.push(Entity::new(
        drawn,
        plane,
        EntityKind::Point { x: 3.0, y: 4.0 },
    ));
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    click(&mut app, &path, Some(drawn));
    assert_eq!(polar_centre(&app, &path, array_id), drawn);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Linear);
    assert!(has_entity(&app, &path, drawn));
}

/// A minted centre something else now uses (here a Line drawn to it) stays.
#[test]
fn a_minted_centre_in_use_stays_when_the_pattern_leaves_polar() {
    let path = PathBuf::from("kind-used-centre.snxfpt");
    let (mut app, array_id) = app_on_a_linear(&path);
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Polar);
    click(&mut app, &path, None);
    let centre = polar_centre(&app, &path, array_id);
    let source = kind(&app, &path, array_id).source();
    let sketch = sketch_mut(&mut app, &path);
    let plane = sketch.planes[0].id;
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line {
            start: centre,
            end: source,
        },
    ));
    app.fp_editor_pattern_kind_picked(array_id, PatternKind::Linear);
    assert!(matches!(
        kind(&app, &path, array_id),
        ArrayKind::Linear { .. }
    ));
    assert!(has_entity(&app, &path, centre));
}
