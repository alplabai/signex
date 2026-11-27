//! Footprint sketch tools — the pattern tools: Linear, Rectangular and
//! Circular Pattern. Each click on a source adds one `Array` to the
//! sketch; its count and spacing are edited afterwards in the Pattern
//! sub-form.

use super::ToolClickCtx;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::tool::{
    LINEAR_PATTERN_DEFAULT_COUNT, LINEAR_PATTERN_DEFAULT_SPACING_MM,
};
use crate::library::editor::footprint::state::{SketchTool, ToolPending};
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;

pub(super) fn apply(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    tool: SketchTool,
) {
    match tool {
        SketchTool::LinearPattern => linear_pattern(editor, ctx),
        SketchTool::RectPattern => rect_pattern(editor, ctx),
        SketchTool::CircularPattern => circular_pattern(editor, ctx),
        _ => unreachable!("non-pattern tool routed here"),
    }
}

/// Add `kind` to the sketch as a new array with sequential numbering and
/// rebuild, so its instances bake. With a pattern selected the new array
/// repeats every instance of it (mockup row 191, decision 10).
fn push_array(editor: &mut crate::app::FootprintDocument, kind: ArrayKind) {
    let array = Array {
        id: ArrayId::new(),
        kind,
        numbering: NumberingScheme::default(),
    };
    if let Some(source) = super::mirror_pattern::selected_pattern(editor) {
        push_linked_array(editor, array, source);
        editor.state.tool_pending = ToolPending::Idle;
        return;
    }
    let sketch = editor
        .primitive_mut()
        .sketch
        .get_or_insert_with(signex_sketch::SketchData::default);
    sketch.arrays.push(array);
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.state.tool_pending = ToolPending::Idle;
}

/// Add `array` repeating every instance of pattern `source`; its own source
/// entity becomes the innermost one. A link that would loop is refused
/// with a warning and adds nothing.
fn push_linked_array(editor: &mut crate::app::FootprintDocument, array: Array, source: ArrayId) {
    use super::mirror_pattern::{link_patterns, refuse};
    let id = array.id;
    let linked = link_patterns(editor, |sketch| {
        sketch.arrays.push(array);
        sketch.set_pattern_source(id, source)
    });
    if let Err(cycle) = linked {
        refuse(editor, format!("Pattern: {cycle} — no pattern was added"));
    }
}

// Linear Pattern (mockup row 191). Click 1 picks the source entity, as Rectangular
// Pattern does. The pitch along X is the dimension input when one is
// typed (a value that is not a positive length is refused, as Fillet
// refuses it), else LINEAR_PATTERN_DEFAULT_SPACING_MM.
fn linear_pattern(editor: &mut crate::app::FootprintDocument, ctx: &ToolClickCtx) {
    let spacing_mm = match super::typed_dimension_input_mm(editor) {
        None => LINEAR_PATTERN_DEFAULT_SPACING_MM,
        Some(super::ToolDimension::Accepted { mm, .. }) => mm,
        Some(super::ToolDimension::Rejected { buffer }) => {
            super::reject_tool_dimension(editor, "Linear Pattern", "pattern spacing", &buffer);
            return;
        }
    };
    push_array(
        editor,
        ArrayKind::Linear {
            source: ctx.resolved_id,
            count_expr: LINEAR_PATTERN_DEFAULT_COUNT.to_string(),
            dx_expr: format!("{spacing_mm}mm"),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
    );
}

// v0.22 Phase B3 — Rectangular Pattern. Click 1 picks the source entity
// (whatever was clicked, including a freshly-minted Point if the click
// missed everything). Mints a default 2×2 grid with 5 mm spacing,
// sequential numbering.
fn rect_pattern(editor: &mut crate::app::FootprintDocument, ctx: &ToolClickCtx) {
    push_array(
        editor,
        ArrayKind::Grid {
            source: ctx.resolved_id,
            nx_expr: "2".into(),
            ny_expr: "2".into(),
            dx_expr: "5mm".into(),
            dy_expr: "5mm".into(),
            depopulation: None,
        },
    );
}

// v0.22 Phase B4 — Circular Pattern. Click 1 picks the source entity.
// The polar array requires a centre Point — mint a fresh one 5 mm to the
// right of the click position so the array doesn't all stack on the
// source. Default count 4, sweep 360°.
fn circular_pattern(editor: &mut crate::app::FootprintDocument, ctx: &ToolClickCtx) {
    let centre_id = SketchEntityId::new();
    let centre = ctx.flag(Entity::new(
        centre_id,
        ctx.plane_id,
        EntityKind::Point {
            x: ctx.x_mm + 5.0,
            y: ctx.y_mm,
        },
    ));
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(centre));
    });
    push_array(
        editor,
        ArrayKind::Polar {
            source: ctx.resolved_id,
            center: centre_id,
            count_expr: "4".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        },
    );
}
