//! The Board section of the Properties panel in board sketch mode (#2234):
//! tag an edge as board outline, a circle as a mounting hole, and edit the
//! hole's drill, plating and net. Each edit is one step of the sketch's
//! undo, like any other sketch attribute edit; the board changes only when
//! the sketch is committed.

use super::super::*;

use signex_sketch::attr::MountingHoleAttr;
use signex_sketch::entity::{Entity, EntityKind};

use super::shape::{rebuild_sketch, sketch_entity_mut};
use crate::panels::BoardSketchAttrMsg;

impl Signex {
    /// Apply one Board section edit to the board sketch in view. Always
    /// handled: with no board sketch in view the edit does nothing.
    pub(in crate::app::handlers::dock::sch_library) fn board_sketch_attr_edited(
        &mut self,
        msg: &BoardSketchAttrMsg,
    ) -> bool {
        use BoardSketchAttrMsg as M;
        let (id, key) = match msg {
            M::OutlineToggled { id, .. }
            | M::MountingHoleToggled { id, .. }
            | M::HolePlatedToggled { id, .. } => (*id, None),
            M::HoleDrillEdited { id, .. } => (*id, Some(("hole_drill", id.0.as_u128()))),
            M::HoleNetEdited { id, .. } => (*id, Some(("hole_net", id.0.as_u128()))),
        };
        if let Some(editor) = self.active_footprint_editor_mut()
            && editor.state.board_backdrop.is_some()
        {
            editor.record_edit(key, |editor| {
                let Some(entity) = sketch_entity_mut(editor, id) else {
                    return false;
                };
                if !apply(entity, msg) {
                    return false;
                }
                rebuild_sketch(editor);
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }
}

/// Write `msg` onto `entity`. Whether it applied: an outline needs an edge,
/// a mounting hole a circle, and the hole fields an existing hole.
fn apply(entity: &mut Entity, msg: &BoardSketchAttrMsg) -> bool {
    use BoardSketchAttrMsg as M;
    let optional = |value: &str| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    };
    match msg {
        M::OutlineToggled { on, .. } => {
            if matches!(entity.kind, EntityKind::Point { .. }) {
                return false;
            }
            entity.board_outline = *on;
        }
        M::MountingHoleToggled { on, .. } => {
            if !matches!(entity.kind, EntityKind::Circle { .. }) {
                return false;
            }
            entity.mounting_hole = on.then(MountingHoleAttr::default);
        }
        M::HoleDrillEdited { value, .. } => {
            let Some(hole) = entity.mounting_hole.as_mut() else {
                return false;
            };
            hole.drill_expr = optional(value);
        }
        M::HolePlatedToggled { on, .. } => {
            let Some(hole) = entity.mounting_hole.as_mut() else {
                return false;
            };
            hole.plated = *on;
        }
        M::HoleNetEdited { value, .. } => {
            let Some(hole) = entity.mounting_hole.as_mut() else {
                return false;
            };
            hole.net = optional(value);
        }
    }
    true
}
