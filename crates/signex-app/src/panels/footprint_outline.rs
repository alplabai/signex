//! What the Footprint sections' illustration draws (mockup row 124): the
//! pads and the straight strokes of the silkscreen and assembly drawings.

use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

/// One pad as the Footprint sections' illustration draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct PadOutlineSummary {
    /// Centre in mm.
    pub position_mm: [f64; 2],
    /// Width and height in mm, before rotation.
    pub size_mm: [f64; 2],
    pub rotation_deg: f64,
    /// The four corners turned by the rotation, in mm.
    pub corners_mm: [[f64; 2]; 4],
    pub shape: signex_library::PadShape,
    /// The pad has a drill hole.
    pub has_drill: bool,
    /// The pad's designator, drawn on it in the illustration.
    pub number: String,
    /// Its template and that template's library; empty when it has none.
    pub template: String,
    pub template_library: String,
}

/// One straight stroke, in mm.
pub type OutlineSegment = [[f64; 2]; 2];

/// The straight strokes of `graphics`: lines, rectangle edges and polygon
/// edges. Arcs, circles and text are left out of the small illustration.
pub fn segments(graphics: &[FpGraphic]) -> Vec<OutlineSegment> {
    let mut out = Vec::new();
    for graphic in graphics {
        match &graphic.kind {
            FpGraphicKind::Line { from, to } => out.push([*from, *to]),
            FpGraphicKind::Rectangle { from, to } => {
                let (a, b) = ([to[0], from[1]], [from[0], to[1]]);
                out.extend([[*from, a], [a, *to], [*to, b], [b, *from]]);
            }
            FpGraphicKind::Polygon { vertices } => {
                let next = vertices.iter().cycle().skip(1);
                out.extend(vertices.iter().zip(next).map(|(a, b)| [*a, *b]));
            }
            FpGraphicKind::Circle { .. }
            | FpGraphicKind::Arc { .. }
            | FpGraphicKind::Text { .. } => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_gives_its_four_edges_and_text_gives_none() {
        let kinds = [
            FpGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [2.0, 1.0],
            },
            FpGraphicKind::Text {
                position: [0.0, 0.0],
                content: "R1".into(),
                size: 1.0,
                frame: None,
            },
        ];
        let graphics: Vec<FpGraphic> = kinds
            .into_iter()
            .map(|kind| FpGraphic::drawn(kind, 0.12, false))
            .collect();
        let edges = segments(&graphics);
        assert_eq!(edges.len(), 4);
        assert_eq!(edges[0], [[0.0, 0.0], [2.0, 0.0]]);
        assert_eq!(edges[3], [[0.0, 1.0], [0.0, 0.0]]);
    }
}
