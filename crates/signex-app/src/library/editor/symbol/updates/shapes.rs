//! The curve shapes of the symbol body placed from their geometry (#2838):
//! the Bezier tool's last click and the `symbol_draw_*` commands commit here,
//! through the same [`push_graphic`] every tool's click uses, so a shape
//! placed by a command is the shape the clicks place: one undo step, the
//! tool's stroke, no fill, on the part and body the editor shows.

use signex_library::SymbolGraphicKind;

use super::{SymEditor, dist_sq, push_graphic};

/// The stroke a curve tool's click gives a new shape (mm).
const CURVE_STROKE_MM: f64 = 0.15;

/// One click of the Bezier gesture. The fourth takes the stash and places
/// the curve as a single undo step; four clicks on one point place nothing,
/// as a degenerate polygon does.
pub(super) fn bezier_click(editor: &mut SymEditor, x: f64, y: f64) {
    editor.polygon_vertices.push((x, y));
    let &[a, b, c, d] = editor.polygon_vertices.as_slice() else {
        editor.canvas_cache.clear();
        return;
    };
    editor.polygon_vertices.clear();
    let points = [a, b, c, d].map(|(x, y)| [x, y]);
    if place_shape(editor, SymbolGraphicKind::Bezier { points }).is_err() {
        editor.canvas_cache.clear();
    }
}

/// Why [`place_shape`] placed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeRefusal {
    /// Not a Bezier, Elliptical Arc, Round Rectangle or Pie.
    NotACurve,
    /// The shape has no visible size, sweeps no angle, or holds a number
    /// that is not finite.
    Degenerate,
}

impl ShapeRefusal {
    /// The refusal as a command answers it.
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::NotACurve => {
                "only a Bezier, elliptical arc, round rectangle or pie is drawn here"
            }
            Self::Degenerate => "the shape has no visible size or sweeps no angle",
        }
    }
}

fn is_curve(kind: &SymbolGraphicKind) -> bool {
    matches!(
        kind,
        SymbolGraphicKind::Bezier { .. }
            | SymbolGraphicKind::Pie { .. }
            | SymbolGraphicKind::EllipticalArc { .. }
            | SymbolGraphicKind::RoundRectangle { .. }
    )
}

/// `kind` as it is stored: the arc and pie angles in `[0, 360)`, read
/// counter-clockwise from the start to the end (the end may be the smaller
/// number: the sweep wraps through 0), and a round rectangle's radius held
/// to half its short side. The click gestures order a dragged pair before
/// they get here (`normalize_arc_commit_deg`).
fn committed(mut kind: SymbolGraphicKind) -> SymbolGraphicKind {
    if let SymbolGraphicKind::Pie {
        start_deg, end_deg, ..
    }
    | SymbolGraphicKind::EllipticalArc {
        start_deg, end_deg, ..
    } = &mut kind
    {
        *start_deg = start_deg.rem_euclid(360.0) + 0.0;
        *end_deg = end_deg.rem_euclid(360.0) + 0.0;
    }
    kind.clamp_corner_radius();
    kind
}

/// Whether the committed `kind` draws nothing. The Pie, Elliptical Arc and
/// Round Rectangle read the shared curve model's test (#1267); a Bezier
/// whose four points sit on one spot draws nothing either.
fn is_degenerate(kind: &SymbolGraphicKind) -> bool {
    match kind {
        SymbolGraphicKind::Bezier { points } => {
            let eps = signex_library::CHAIN_ENDPOINT_EPSILON_MM;
            !points.iter().flatten().all(|n| n.is_finite())
                || points.iter().all(|&p| dist_sq(p, points[0]) <= eps * eps)
        }
        other => other
            .curve_shape()
            .is_none_or(|shape| shape.is_degenerate()),
    }
}

/// Place the curve `kind` on the symbol as one undo step and answer the
/// index of the new graphic. Refused, with nothing changed, for another kind
/// or a shape that draws nothing.
pub(crate) fn place_shape(
    editor: &mut SymEditor,
    kind: SymbolGraphicKind,
) -> Result<usize, ShapeRefusal> {
    if !is_curve(&kind) {
        return Err(ShapeRefusal::NotACurve);
    }
    let kind = committed(kind);
    if is_degenerate(&kind) {
        return Err(ShapeRefusal::Degenerate);
    }
    push_graphic(editor, kind, CURVE_STROKE_MM);
    Ok(editor.primitive().graphics.len() - 1)
}
