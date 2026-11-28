//! The click that answers an armed centre pick: the Pattern form's Re-pick
//! centre, or its Polar kind segment (#2596). It runs before any tool's own
//! click handling (#3232): a click naming a Point picks that Point, any
//! other click mints the centre at the click. A pick that can no longer
//! apply (the pattern is gone, or is already Polar) mints nothing.

use signex_bake::array::{KindTarget, PatternKind, kind_switch_refusal};
use signex_sketch::array::ArrayKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::ToolPending;

use super::super::pattern_kind;

/// Answer the armed centre pick with the click at `at` (on Point
/// `snap_id`, when it names one). Returns whether a pick was armed, so the
/// click is taken.
pub(super) fn try_answer(
    editor: &mut FootprintDocument,
    at: (f64, f64),
    snap_id: Option<SketchEntityId>,
) -> bool {
    let ToolPending::RepickPolarCenter {
        array_id,
        is_kind_switch,
    } = editor.state.tool_pending
    else {
        return false;
    };
    editor.state.tool_pending = ToolPending::Idle;
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return true;
    };
    if is_kind_switch {
        if let Some(refused) = kind_switch_refusal(sketch, array_id, PatternKind::Polar) {
            pattern_kind::report_refused(PatternKind::Polar, refused);
            return true;
        }
    } else if !sketch
        .arrays
        .iter()
        .any(|a| a.id == array_id && matches!(a.kind, ArrayKind::Polar { .. }))
    {
        return true;
    }
    let picked = snap_id.filter(|id| {
        sketch
            .entities
            .iter()
            .any(|e| e.id == *id && matches!(e.kind, EntityKind::Point { .. }))
    });
    let centre = picked.unwrap_or_else(|| mint_centre(editor, at));
    if is_kind_switch {
        // The centre the click minted goes again when the pattern leaves
        // Polar.
        if picked.is_none() {
            editor.state.switch_minted_centres.insert(centre);
        }
        pattern_kind::switch_kind(editor, array_id, KindTarget::Polar { centre });
        return true;
    }
    if let Some(sketch) = editor.primitive_mut().sketch.as_mut()
        && let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id)
        && let ArrayKind::Polar { center, .. } = &mut array.kind
    {
        *center = centre;
    }
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.mark_dirty();
    true
}

/// A new Point at `at` for the centre, with the sticky construction and
/// centerline flags as any click Point.
fn mint_centre(editor: &mut FootprintDocument, (x, y): (f64, f64)) -> SketchEntityId {
    let plane = super::resolve_sketch_plane(editor);
    let id = SketchEntityId::new();
    let mut entity = Entity::new(id, plane, EntityKind::Point { x, y });
    entity.construction = editor.state.construction_mode;
    entity.centerline = editor.state.centerline_mode;
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(entity));
    });
    id
}
