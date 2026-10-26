//! Pad-stack health of a footprint opened in the editor (#1073).
//!
//! Before #1052, `PadStackOverrides` derived `Default`, which made
//! `paste_top_enabled`, `paste_bottom_enabled` and `thermal_relief` all
//! `false` (their serde defaults are `true`). Every sketch pad minted in
//! that window was saved with paste off on both sides and direct connect,
//! so the bake emits no paste stencil openings for it. An explicit `false`
//! in a saved file cannot be told from a deliberate choice, so nothing here
//! rewrites a pad on its own:
//!
//! - [`legacy_default_pads`] finds a footprint whose every sketch pad still
//!   carries exactly that derived-default stack; the Properties panel offers
//!   to restore it, and [`restore_pad_stack_defaults`] does so as one undo
//!   step the user still has to save.
//! - [`smd_pads_without_paste`] is the standing check: SMD pads whose copper
//!   side has no paste opening.

use signex_sketch::attr::PadStackOverrides;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

use super::layers::FpLayer;
use super::state::EditorPad;

/// True when `stack` is the all-false stack the pre-#1052 derived `Default`
/// produced: paste off on both sides and direct connect, every other
/// override at its default. `corner_radius_pct` is ignored: the Pads-mode
/// mirror may legitimately have written it and it is not part of the defect.
pub fn is_pre_1052_derived_default(stack: &PadStackOverrides) -> bool {
    let affected = PadStackOverrides {
        paste_top_enabled: false,
        paste_bottom_enabled: false,
        thermal_relief: false,
        corner_radius_pct: stack.corner_radius_pct,
        ..PadStackOverrides::default()
    };
    *stack == affected
}

/// Sketch pads of a footprint that carry the pre-#1052 derived-default
/// stack, but only when EVERY sketch pad does. A single pad with any other
/// stack means the user (or a newer build) has set these flags on purpose,
/// so no offer is made. Empty when the sketch has no pads.
pub fn legacy_default_pads(sketch: &SketchData) -> Vec<SketchEntityId> {
    let mut ids = Vec::new();
    for entity in &sketch.entities {
        let Some(attr) = entity.pad.as_ref() else {
            continue;
        };
        if !is_pre_1052_derived_default(&attr.stack) {
            return Vec::new();
        }
        ids.push(entity.id);
    }
    ids
}

/// Numbers of the SMD pads whose copper side has paste disabled, in pad
/// order. A pad on the bottom copper layer is judged by its bottom flag,
/// every other SMD pad by its top flag.
pub fn smd_pads_without_paste(pads: &[EditorPad]) -> Vec<String> {
    pads.iter()
        .filter(|pad| matches!(pad.kind, signex_library::primitive::footprint::PadKind::Smd))
        .filter(|pad| {
            if matches!(pad.primary_layer(), FpLayer::BCu) {
                !pad.stack.paste_enabled_bottom
            } else {
                !pad.stack.paste_enabled_top
            }
        })
        .map(|pad| {
            if pad.number.is_empty() {
                "(unnumbered)".to_string()
            } else {
                pad.number.clone()
            }
        })
        .collect()
}

/// Enable paste on both sides and thermal relief on every sketch pad that
/// [`legacy_default_pads`] flags, as one undo step. Returns whether
/// anything changed. The editor is left dirty; nothing is written to disk
/// until the user saves.
pub fn restore_pad_stack_defaults(editor: &mut crate::app::FootprintEditorState) -> bool {
    editor.record_edit(None, |editor| {
        let ids = editor
            .primitive()
            .sketch
            .as_ref()
            .map(legacy_default_pads)
            .unwrap_or_default();
        if ids.is_empty() {
            return false;
        }
        editor.with_parts(|state, primitive| {
            for pad in &mut state.pads {
                if pad.sketch_entity_id.is_some_and(|id| ids.contains(&id)) {
                    pad.stack.paste_enabled_top = true;
                    pad.stack.paste_enabled_bottom = true;
                    pad.stack.thermal_relief = true;
                }
            }
            super::state::FootprintEditorState::sync_pads_to_primitive(state, primitive);
        });
        editor.dirty = true;
        editor.canvas_cache.clear();
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::PadAttr;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::plane::PlaneId;

    fn legacy_stack() -> PadStackOverrides {
        PadStackOverrides {
            paste_top_enabled: false,
            paste_bottom_enabled: false,
            thermal_relief: false,
            ..PadStackOverrides::default()
        }
    }

    fn sketch_with(stacks: Vec<PadStackOverrides>) -> SketchData {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        for (i, stack) in stacks.into_iter().enumerate() {
            let mut e = Entity::new(
                SketchEntityId::new(),
                plane,
                EntityKind::Point {
                    x: i as f64,
                    y: 0.0,
                },
            );
            e.pad = Some(PadAttr {
                number: (i + 1).to_string(),
                stack,
                ..PadAttr::default()
            });
            sketch.entities.push(e);
        }
        sketch
    }

    #[test]
    fn every_pad_all_false_is_flagged() {
        let sketch = sketch_with(vec![legacy_stack(), legacy_stack()]);
        assert_eq!(legacy_default_pads(&sketch).len(), 2);
    }

    #[test]
    fn one_deliberate_pad_withdraws_the_offer() {
        let sketch = sketch_with(vec![legacy_stack(), PadStackOverrides::default()]);
        assert!(legacy_default_pads(&sketch).is_empty());
    }

    #[test]
    fn a_pad_with_a_margin_set_is_not_the_derived_default() {
        let custom = PadStackOverrides {
            paste_top_expr: Some("-0.04mm".into()),
            ..legacy_stack()
        };
        assert!(legacy_default_pads(&sketch_with(vec![custom])).is_empty());
    }

    #[test]
    fn a_footprint_without_sketch_pads_is_not_flagged() {
        assert!(legacy_default_pads(&SketchData::default()).is_empty());
    }
}
