//! Object hotspots of the footprint editor: the origin, pads (centres,
//! vertices, edges), silk graphics (vertices, lines, arc centres, texts,
//! regions) and the 3D body, each behind its row of Objects for snapping,
//! within the Snap Distance, plus the Axes alignment within Axis Snap Range
//! (#2726). A hotspot places the cursor only. Pads mode reaches every
//! object; Sketch mode only the pads and silk the sketch does not draw,
//! since its own Points snap through its own chain (`docs/FOOTPRINT_SKETCH.md`).

mod body;
mod pad_ring;
mod points;

#[cfg(test)]
mod tests;

pub use body::BodyBox;

use signex_library::primitive::footprint::FpGraphic;

use super::snap::{SnapKind, SnapResult};
use super::state::FootprintEditorState;
use points::{Points, candidates};

/// What the canvas offers the object snap beyond the editor state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObjectTargets<'a> {
    pub silk_f: &'a [FpGraphic],
    pub silk_b: &'a [FpGraphic],
    /// The 3D body's box; `None` where the canvas shows no footprint body.
    pub body: Option<BodyBox>,
    /// The pad being dragged: it never snaps to itself.
    pub moving_pad: Option<usize>,
    /// The drag carries the selection: no selected pad is a target either.
    pub moving_selection: bool,
}

/// The nearest enabled hotspot within the Snap Distance of `raw`, or `None`
/// outside Pads and Sketch mode or with nothing in reach.
pub fn object_snap(
    raw: (f64, f64),
    state: &FootprintEditorState,
    targets: ObjectTargets<'_>,
) -> Option<SnapResult> {
    let reach = state.snap_options.snap_distance_mm.max(1e-6);
    candidates(raw, state, targets, Points::All)
        .into_iter()
        .map(|p| (distance(p, raw), p))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, pos)| SnapResult {
            pos,
            kind: SnapKind::Object,
        })
}

/// The x and the y that Axes lines `raw` up with: of the fixed hotspots
/// within Axis Snap Range, the nearest whose x is within the Snap Distance
/// of the pointer's x gives the x, the same for y; `None` on an axis
/// nothing lines up on. The nearest point of a line or an edge is left out,
/// since it would line the pointer up with itself (as on a board, #2879).
pub fn axis_snap(
    raw: (f64, f64),
    state: &FootprintEditorState,
    targets: ObjectTargets<'_>,
) -> (Option<f64>, Option<f64>) {
    let opts = state.snap_options;
    if !opts.snap_to_axes {
        return (None, None);
    }
    let reach = opts.snap_distance_mm.max(1e-6);
    let range = opts.axis_snap_range_mm;
    let near: Vec<(f64, (f64, f64))> = candidates(raw, state, targets, Points::Fixed)
        .into_iter()
        .map(|p| (distance(p, raw), p))
        .filter(|(d, _)| *d <= range)
        .collect();
    let along = |gap: fn((f64, f64), (f64, f64)) -> f64, value: fn((f64, f64)) -> f64| {
        near.iter()
            .filter(|(_, p)| gap(*p, raw) <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| value(*p))
    };
    (
        along(|p, q| (p.0 - q.0).abs(), |p| p.0),
        along(|p, q| (p.1 - q.1).abs(), |p| p.1),
    )
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
