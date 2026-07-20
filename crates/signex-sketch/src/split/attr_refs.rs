//! Rewrites the retired Line's id wherever it appears OUTSIDE
//! `entities` / `constraints` — `SketchData::arrays` and the pad
//! Custom-shape / Custom-paste-aperture profile-seed lists nested
//! inside `Entity::pad`. See [`super::split_line`]'s doc comment for
//! the per-collection carry-over rule these implement.

use crate::array::ArrayKind;
use crate::attr::{CustomPadShape, PadShape, PasteAperturePattern};
use crate::id::SketchEntityId;
use crate::sketch::SketchData;

use super::SplitCtx;

/// An array's `source` (and Polar's `center`) must resolve to a Point
/// carrying a `PadAttr` to bake at all — `bake_one_pad` looks it up
/// via `point_xy`, which a Line id never satisfies. So this only fires
/// on sketch data that was already malformed before the split; it is
/// still rewritten rather than left dangling, the same judgment call
/// `constraints::point_param` makes for an unresolvable point.
pub(super) fn retarget_arrays(sketch: &mut SketchData, ctx: &SplitCtx) {
    for array in &mut sketch.arrays {
        match &mut array.kind {
            ArrayKind::Linear { source, .. } | ArrayKind::Grid { source, .. } => {
                retarget(source, ctx);
            }
            ArrayKind::Polar { source, center, .. } => {
                retarget(source, ctx);
                retarget(center, ctx);
            }
        }
    }
}

/// `CustomPadShape::SketchProfile.source` / `PasteAperturePattern::
/// Custom.source` are seed lists into `trace_closed_profile`'s
/// adjacency walk (today only `source[0]` is read — see
/// `signex-bake/src/pad.rs`). The walk discovers the WHOLE closed loop
/// from any edge on it, and `line_a` is still wired into that same
/// loop (through `start`, and through the new mid Point to `line_b`),
/// so re-seeding with `line_a` re-finds the identical profile. Every
/// occurrence is rewritten, not just a first entry, in case a future
/// consumer reads more of the list than `source[0]`.
pub(super) fn retarget_pad_profiles(sketch: &mut SketchData, ctx: &SplitCtx) {
    for entity in &mut sketch.entities {
        let Some(pad) = entity.pad.as_mut() else {
            continue;
        };
        if let PadShape::Custom(CustomPadShape::SketchProfile { source }) = &mut pad.shape {
            source.iter_mut().for_each(|id| retarget(id, ctx));
        }
        if let PasteAperturePattern::Custom { source } = &mut pad.paste_apertures {
            source.iter_mut().for_each(|id| retarget(id, ctx));
        }
    }
}

fn retarget(id: &mut SketchEntityId, ctx: &SplitCtx) {
    if *id == ctx.line {
        *id = ctx.line_a;
    }
}
