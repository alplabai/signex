//! "This pad / All pads" for an edit of a patterned pad (mockup row 192 D1,
//! decision 9, Caner 2026-11-20). [`hold_patterned_pad_edit`] holds the
//! edit at the Pads-mode commit point; [`apply`] carries out the answer as
//! one undo step and re-bakes. The question comes back on every such edit.
//!
//! This pad stores only the attributes the edit changed as an override of
//! that instance; All pads writes them onto the pattern's source pad, so
//! every instance changes. The held value never enters the pad list, so no
//! persist path can write it. See docs/FOOTPRINT_SKETCH.md, "Per-pad
//! overrides".

use signex_sketch::array::ArrayId;
use signex_sketch::attr::PadAttr;
use signex_sketch::entity::Entity;
use signex_sketch::sketch::{PadAttrOverride, SketchData};
use signex_types::sketch::SketchHost;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::pad_to_sketch::mirror_pad_attrs_into_sketch;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::editor::footprint::state::EditorPad;
use crate::library::editor::footprint::state::pad_scope::{PadScope, PadScopePrompt};

/// A pad field edit, as the Pads-mode commit point runs it.
pub(crate) type PadEdit<'f> = Box<dyn FnOnce(&mut EditorPad) + 'f>;

/// The instance key of an override.
type InstanceKey = (ArrayId, u32, u32);

/// Hold an edit of pad `idx` when the pad is a patterned instance and the
/// edit changes an attribute an override can hold: the question opens and
/// `None` comes back. Otherwise the edit comes back to run as before.
pub(crate) fn hold_patterned_pad_edit<'f>(
    editor: Option<&mut FootprintDocument>,
    idx: usize,
    edit: impl FnOnce(&mut EditorPad) + 'f,
) -> Option<PadEdit<'f>> {
    let Some(editor) = editor else {
        return Some(Box::new(edit));
    };
    let Some(current) = editor.state.pads.get(idx).cloned() else {
        return Some(Box::new(edit));
    };
    // A question still open on this pad, as it was held, gathers the next
    // edit; one open on another pad, or on a pad that has moved on since,
    // is answered Cancel (dropped: the pad list never held its value).
    let prior = editor
        .state
        .pad_scope
        .take()
        .filter(|prompt| prompt.pad_idx == idx && prompt.before == current);
    let mut after = prior
        .as_ref()
        .map_or_else(|| current.clone(), |p| p.after.clone());
    edit(&mut after);
    let instance = prior
        .as_ref()
        .map(|p| p.instance)
        .or_else(|| patterned_instance(editor, &current));
    let before = current;
    let delta = instance.and_then(|key| attr_delta(editor, key, &before, &after));
    let (Some(instance), Some(delta)) = (instance, delta) else {
        return Some(Box::new(move |pad: &mut EditorPad| *pad = after));
    };
    if delta.is_empty() {
        // A size or a position, or typed back to where it was: no question.
        return Some(Box::new(move |pad: &mut EditorPad| *pad = after));
    }
    editor.state.pad_scope = Some(Box::new(PadScopePrompt {
        pad_idx: idx,
        instance,
        before,
        after,
    }));
    editor.canvas_cache.clear();
    None
}

/// Carry out the answer to the open question.
pub(super) fn apply(editor: &mut FootprintDocument, scope: PadScope) {
    let Some(prompt) = editor.state.pad_scope.take() else {
        return;
    };
    let delta = attr_delta(editor, prompt.instance, &prompt.before, &prompt.after)
        .filter(|d| !d.is_empty());
    match (scope, delta) {
        (PadScope::Cancel, _) | (_, None) => {}
        (PadScope::ThisPad, Some(delta)) => {
            let (array, i, j) = prompt.instance;
            record_sketch_step(editor, |sketch| {
                let mut held = sketch
                    .override_for(array, i, j)
                    .cloned()
                    .unwrap_or_default();
                held.merge(&delta);
                sketch.set_override(array, i, j, held);
            });
        }
        (PadScope::AllPads, Some(delta)) => {
            let pattern = prompt.instance.0;
            record_sketch_step(editor, |sketch| {
                let source = sketch.innermost_source(pattern);
                if let Some(attr) = sketch
                    .entities
                    .iter_mut()
                    .find(|e| Some(e.id) == source)
                    .and_then(|e| e.pad.as_mut())
                {
                    delta.apply_to(attr);
                }
            });
        }
    }
    editor.canvas_cache.clear();
}

/// The pattern instance `pad` is, when it is one other than a source pad.
fn patterned_instance(editor: &FootprintDocument, pad: &EditorPad) -> Option<InstanceKey> {
    if editor.state.sketch_host() != SketchHost::Footprint {
        return None;
    }
    let sketch = editor.primitive().sketch.as_ref()?;
    if sketch.arrays.is_empty() {
        return None;
    }
    let fresh;
    let solve = match editor.state.last_solve.as_ref() {
        Some(solve) => solve,
        None => {
            let params = signex_sketch::parameter::resolve(&sketch.parameters).ok()?;
            fresh = editor.state.sketch_solver.solve(sketch, &params).ok()?;
            &fresh
        }
    };
    let key = signex_bake::instance_at(sketch, solve, pad.position_mm)?;
    let is_source = (key.1, key.2) == (0, 0)
        && sketch.arrays.iter().any(|a| a.id == key.0)
        && sketch.pattern_source_of(key.0).is_none();
    (!is_source).then_some(key)
}

/// What the edit changed, as an override of the pattern's source pad.
fn attr_delta(
    editor: &FootprintDocument,
    instance: InstanceKey,
    before: &EditorPad,
    after: &EditorPad,
) -> Option<PadAttrOverride> {
    let sketch = editor.primitive().sketch.as_ref()?;
    let source = sketch.innermost_source(instance.0)?;
    let entity = sketch.entities.iter().find(|e| e.id == source)?;
    let old = mirrored_attr(entity, before)?;
    let new = mirrored_attr(entity, after)?;
    Some(PadAttrOverride::between(&old, &new))
}

/// The source pad's attributes as the Pads-mode mirror writes `pad` onto
/// them.
fn mirrored_attr(entity: &Entity, pad: &EditorPad) -> Option<PadAttr> {
    let mut scratch = SketchData {
        entities: vec![entity.clone()],
        ..SketchData::default()
    };
    let mut pad = pad.clone();
    pad.sketch_entity_id = Some(entity.id);
    mirror_pad_attrs_into_sketch(std::slice::from_ref(&pad), &mut scratch);
    scratch.entities.pop().and_then(|e| e.pad)
}

/// Change the sketch as one undo step, solved and baked; a change the
/// solver rejects is reverted and leaves no step.
fn record_sketch_step(editor: &mut FootprintDocument, change: impl FnOnce(&mut SketchData)) {
    editor.record_edit(None, |editor| {
        let before = editor.primitive().sketch.clone();
        let result = editor.with_parts(|state, fp| {
            mutate_solve_or_revert(state, fp, |fp| {
                if let Some(sketch) = fp.sketch.as_mut() {
                    change(sketch);
                }
            })
        });
        if let Err(error) = &result {
            report_solve_failure(&mut editor.state, error);
        }
        let changed = editor.primitive().sketch != before;
        if changed {
            editor.mark_dirty();
        }
        changed
    });
}
