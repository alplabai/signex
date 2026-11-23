//! Object hotspots of the footprint editor in Pads mode: pad centres, track
//! vertices, arc centres and the footprint origin, each behind its row of
//! Objects for snapping, within the Snap Distance. A hotspot places the
//! cursor only. Sketch mode keeps its own chain (`docs/FOOTPRINT_SKETCH.md`).

use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::snap::{SnapKind, SnapResult};
use super::state::{EditorMode, FootprintEditorState};

/// What the canvas offers the object snap beyond the editor state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObjectTargets<'a> {
    pub silk_f: &'a [FpGraphic],
    pub silk_b: &'a [FpGraphic],
    /// The pad being dragged: it never snaps to itself.
    pub moving_pad: Option<usize>,
}

/// The nearest enabled hotspot within the Snap Distance of `raw`, or `None`
/// outside Pads mode or with nothing in reach.
pub fn object_snap(
    raw: (f64, f64),
    state: &FootprintEditorState,
    targets: ObjectTargets<'_>,
) -> Option<SnapResult> {
    if state.mode != EditorMode::Normal {
        return None;
    }
    let opts = state.snap_options;
    let reach = opts.snap_distance_mm.max(1e-6);
    let mut best: Option<(f64, (f64, f64))> = None;
    let mut offer = |p: (f64, f64)| {
        let d = (p.0 - raw.0).hypot(p.1 - raw.1);
        if d <= reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, p));
        }
    };
    if opts.snap_footprint_origins {
        offer((0.0, 0.0));
    }
    if opts.snap_pad_centers {
        for (idx, pad) in state.pads.iter().enumerate() {
            if targets.moving_pad != Some(idx) {
                offer(pad.position_mm);
            }
        }
    }
    for graphic in targets.silk_f.iter().chain(targets.silk_b) {
        match &graphic.kind {
            FpGraphicKind::Line { from, to } if opts.snap_track_vertices => {
                offer((from[0], from[1]));
                offer((to[0], to[1]));
            }
            FpGraphicKind::Rectangle { from, to } if opts.snap_track_vertices => {
                offer((from[0], from[1]));
                offer((to[0], from[1]));
                offer((to[0], to[1]));
                offer((from[0], to[1]));
            }
            FpGraphicKind::Polygon { vertices } if opts.snap_track_vertices => {
                for v in vertices {
                    offer((v[0], v[1]));
                }
            }
            FpGraphicKind::Circle { center, .. } | FpGraphicKind::Arc { center, .. }
                if opts.snap_arc_centers =>
            {
                offer((center[0], center[1]));
            }
            _ => {}
        }
    }
    best.map(|(_, pos)| SnapResult {
        pos,
        kind: SnapKind::Object,
    })
}
