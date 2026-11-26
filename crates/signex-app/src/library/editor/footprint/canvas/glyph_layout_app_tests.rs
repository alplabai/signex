//! #3014: a dimension's value on the drawing, laid out from a footprint
//! editor that `Signex::update` drove, in the words the app rendered from
//! the catalog: `name · value` when it is bound to a parameter, the value
//! alone when it is not (mockup row 187).

use iced::Point;
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::{GlyphFace, sketch_glyphs};
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::editor::footprint::EditorMode;
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchEdit};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Two 4 mm sides from a fixed corner: one bound to `side`, one literal.
fn sketch() -> (SketchData, ConstraintId, ConstraintId) {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    data.parameters.insert("side", "4mm");
    let mut add = |x, y| {
        let id = SketchEntityId::new();
        data.entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let (a, b, c) = (add(-2.0, -2.0), add(2.0, -2.0), add(2.0, 2.0));
    let (named, unnamed) = (ConstraintId::new(), ConstraintId::new());
    let kinds = [
        (ConstraintId::new(), ConstraintKind::Fixed { point: a }),
        (
            named,
            ConstraintKind::DistancePtPt {
                p1: a,
                p2: b,
                target: DimTarget::Expr("side".into()),
            },
        ),
        (
            unnamed,
            ConstraintKind::DistancePtPt {
                p1: b,
                p2: c,
                target: DimTarget::Literal(4.0),
            },
        ),
    ];
    data.constraints
        .extend(kinds.map(|(id, kind)| Constraint { id, kind }));
    (data, named, unnamed)
}

/// A footprint tab in sketch mode holding `data`, after one update.
fn sketching(data: SketchData) -> (Signex, std::path::PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = std::path::PathBuf::from("labels.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.primitive_mut().sketch = Some(data);
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.mark_saved();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchCanvas(
            SketchCanvasMsg::MenuClosed,
        )),
    }));
    (app, path)
}

fn value_text(glyphs: &[super::Glyph], id: ConstraintId) -> String {
    let glyph = glyphs.iter().find(|g| g.constraint() == Some(id));
    match &glyph.expect("a glyph").face {
        GlyphFace::Value { text, .. } => text.clone(),
        other => panic!("a value: {other:?}"),
    }
}

#[test]
fn a_bound_dimension_reads_name_and_value_and_an_unnamed_one_its_value() {
    let (data, named, unnamed) = sketch();
    let (app, path) = sketching(data);
    let editor = &app.document_state.footprint_editors[&path];
    assert_eq!(editor.state.canvas_text.unit_mm, "mm", "words rendered");
    let sketch = editor.primitive().sketch.as_ref().expect("a sketch");
    let cstate = FootprintCanvasState::with_view(100.0, Point::new(400.0, 300.0));
    let glyphs = sketch_glyphs(sketch, &editor.state, &cstate);
    assert_eq!(value_text(&glyphs, named), "side · 4.00 mm");
    assert_eq!(value_text(&glyphs, unnamed), "4.00 mm");
}
