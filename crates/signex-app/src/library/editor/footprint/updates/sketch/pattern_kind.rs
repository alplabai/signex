//! Switching a footprint pattern's kind from the Properties kind segment
//! (#2596). Linear and Grid switch at once, as one undo step. Polar first
//! needs a centre: picking it arms the Re-pick centre flow
//! (`ToolPending::RepickPolarCenter` with `is_kind_switch`), the click that
//! answers it switches the pattern (one undo step with the click), and Esc
//! leaves the pattern as it was. The reshaping itself is
//! `signex_bake::array::switch_pattern_kind`; see docs/FOOTPRINT_SKETCH.md,
//! "Switching a pattern's kind".

use signex_bake::array::{KindSwitched, KindTarget, PatternKind, switch_pattern_kind};
use signex_i18n::{I18n, ids};
use signex_sketch::array::ArrayId;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::ToolPending;

/// The kind segment of `array_id` was picked. Linear and Grid switch in one
/// undo step; Polar arms the centre pick; the pattern's own kind cancels a
/// centre pick still waiting. Returns whether the pattern or the pick changed.
pub(crate) fn kind_picked(
    editor: &mut FootprintDocument,
    array_id: ArrayId,
    kind: PatternKind,
) -> bool {
    let current = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| s.arrays.iter().find(|a| a.id == array_id))
        .map(|a| PatternKind::of(&a.kind));
    let Some(current) = current else {
        return false;
    };
    let cancelled = cancel_centre_pick(editor, array_id);
    if kind == current {
        return cancelled;
    }
    let target = match kind {
        PatternKind::Linear => KindTarget::Linear,
        PatternKind::Grid => KindTarget::Grid,
        PatternKind::Polar => {
            editor.state.tool_pending = ToolPending::RepickPolarCenter {
                array_id,
                is_kind_switch: true,
            };
            editor.canvas_cache.clear();
            return true;
        }
    };
    editor.record_edit(None, |editor| switch_kind(editor, array_id, target)) || cancelled
}

/// Drop a centre pick that would switch `array_id` to Polar. Returns
/// whether one was waiting.
fn cancel_centre_pick(editor: &mut FootprintDocument, array_id: ArrayId) -> bool {
    let waiting = matches!(
        editor.state.tool_pending,
        ToolPending::RepickPolarCenter { array_id: id, is_kind_switch: true } if id == array_id
    );
    if waiting {
        editor.state.tool_pending = ToolPending::Idle;
        editor.canvas_cache.clear();
    }
    waiting
}

/// Switch `array_id` to `target` and re-solve, naming in the Messages
/// panel what had no place in the new kind. Records no undo step: the
/// caller's edit does. Returns whether the pattern changed.
pub(crate) fn switch_kind(
    editor: &mut FootprintDocument,
    array_id: ArrayId,
    target: KindTarget,
) -> bool {
    let Some(sketch) = editor.primitive_mut().sketch.as_mut() else {
        return false;
    };
    let Ok(switched) = switch_pattern_kind(sketch, array_id, target) else {
        return false;
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.mark_dirty();
    editor.canvas_cache.clear();
    report_dropped(target.kind(), &switched);
    true
}

/// The Messages lines naming the suppressed instances and pad overrides a
/// switch dropped (DECISIONS 2026-11-27, decision 3). The log stays English
/// under any UI language.
fn report_dropped(kind: PatternKind, switched: &KindSwitched) {
    for line in dropped_lines(&I18n::en_us(), kind, switched) {
        crate::diagnostics::log_warning(line);
    }
}

pub(super) fn dropped_lines(
    i18n: &I18n,
    kind: PatternKind,
    switched: &KindSwitched,
) -> Vec<String> {
    let name = i18n.message_plain(kind_label(kind), &[]);
    [
        (
            ids::EDITORS_ROLE_PATTERN_KIND_DROPPED_SKIPS,
            &switched.suppressed_dropped,
        ),
        (
            ids::EDITORS_ROLE_PATTERN_KIND_DROPPED_OVERRIDES,
            &switched.overrides_dropped,
        ),
    ]
    .into_iter()
    .filter(|(_, cells)| !cells.is_empty())
    .map(|(id, cells)| {
        let instances = cells
            .iter()
            .map(|(i, j)| format!("({i}, {j})"))
            .collect::<Vec<_>>()
            .join(", ");
        i18n.message_plain(
            id,
            &[
                ("kind", name.as_str().into()),
                ("instances", instances.into()),
            ],
        )
    })
    .collect()
}

/// The kind segment's label of `kind`.
fn kind_label(kind: PatternKind) -> signex_i18n::MessageId {
    match kind {
        PatternKind::Linear => ids::EDITORS_ROLE_PATTERN_KIND_LINEAR,
        PatternKind::Grid => ids::EDITORS_ROLE_PATTERN_KIND_GRID,
        PatternKind::Polar => ids::EDITORS_ROLE_PATTERN_KIND_POLAR,
    }
}
