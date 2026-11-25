//! Mockup row 179e (#2894, #2772) through `Signex::update`: a 2 x 2 hole
//! pattern spaced by the size parameters. Its glyph opens the Rectangular
//! pattern form, Parameters lists the names it uses, the solve warns of no
//! pad, and the bake preview writes a hole per instance.

use iced::Size;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::MountingHoleAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::tests::{send, sketch_edit, sketching_off_grid};
use crate::app::{BoardSketchMsg, Message, PcbMsg, Signex};
use crate::layout_probe;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchEdit};
use crate::library::messages::FootprintEditorMsg;

/// A mounting hole circle at (3.5, 3.5) and its 2 x 2 pattern spaced by
/// `board_w - 2*hole_inset` and `board_h - 2*hole_inset`.
fn hole_pattern() -> (SketchData, ArrayId) {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    data.parameters.insert("board_w", "60mm");
    data.parameters.insert("board_h", "40mm");
    data.parameters.insert("hole_inset", "3.5mm");
    let center = SketchEntityId::new();
    data.entities.push(Entity::new(
        center,
        plane,
        EntityKind::Point { x: 3.5, y: 3.5 },
    ));
    let circle = SketchEntityId::new();
    let mut hole = Entity::new(
        circle,
        plane,
        EntityKind::Circle {
            center,
            radius: 1.6,
        },
    );
    hole.mounting_hole = Some(MountingHoleAttr::default());
    data.entities.push(hole);
    let pattern = ArrayId::new();
    data.arrays.push(Array {
        id: pattern,
        kind: ArrayKind::Grid {
            source: circle,
            nx_expr: "2".into(),
            ny_expr: "2".into(),
            dx_expr: "board_w - 2*hole_inset".into(),
            dy_expr: "board_h - 2*hole_inset".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    (data, pattern)
}

/// The board in sketch mode on [`hole_pattern`], with the pattern's glyph
/// clicked.
fn pattern_selected() -> (Signex, tempfile::TempDir) {
    let (mut app, tmp) = sketching_off_grid();
    let path = app.active_tab_path().expect("the board tab");
    let (data, pattern) = hole_pattern();
    {
        let session = app.interaction_state.board_sketch.as_mut().expect("on");
        let editor = &mut session.editor;
        editor.primitive_mut().sketch = Some(data);
        editor.with_parts(|state, primitive| {
            apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
        });
    }
    sketch_edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::PatternSelected(Some(pattern))),
    );
    (app, tmp)
}

fn properties_texts(app: &Signex) -> Vec<String> {
    let ctx = &app.document_state.panel_ctx;
    let view = crate::panels::view_panel(crate::panels::PanelKind::Properties, ctx);
    layout_probe::lay_out_texts(view, Size::new(360.0, 6000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn a_board_hole_pattern_glyph_opens_the_rectangular_pattern_form() {
    let (app, _tmp) = pattern_selected();
    let texts = properties_texts(&app);
    for shown in [
        "Rectangular pattern",
        "Circle · Mounting hole",
        "53.00 mm",
        "33.00 mm",
        "Bakes into 4 holes",
        "board_w",
        "board_h",
        "hole_inset",
    ] {
        assert!(texts.iter().any(|t| t == shown), "{shown:?}: {texts:#?}");
    }
}

#[test]
fn a_board_hole_pattern_bakes_a_hole_per_instance_without_a_pad_warning() {
    let (mut app, _tmp) = pattern_selected();
    {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        let warnings = &session.editor.state.solve_warnings;
        assert!(
            warnings
                .iter()
                .all(|w| !w.contains("not a pad") && !w.contains("array skipped")),
            "a board pattern is not a pad pattern: {warnings:?}"
        );
    }
    send(
        &mut app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::BakePreviewToggled)),
    );
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    match session.preview.as_ref() {
        Some(crate::panels::BoardBakePreview::Writes(rows)) => {
            assert_eq!(rows.holes.len(), 4, "{rows:?}");
        }
        other => panic!("the preview writes: {other:?}"),
    }
}
