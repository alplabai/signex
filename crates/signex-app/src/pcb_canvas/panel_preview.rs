//! The panel preview (#2410): with a panel set, the board backdrop draws the
//! panel round the board: the frame, the rails, the other copies as board
//! shapes, the routed edges and V-score lines, and the fiducials, tooling
//! holes and mouse-bite holes as rings. The copies show their shape only;
//! the fabrication outputs carry their content. The geometry is
//! `pcb_geom::panel::layout`, worked out once when the board changes.

use iced::widget::canvas::path::Builder;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point};
use pcb_geom::nm_to_mm;
use pcb_geom::panel::{PanelLayout, layout};
use signex_types::pcb::PcbBoard;
use signex_types::theme::CanvasColors;

use super::camera::BoardCamera;

const PANEL_LINE_PX: f32 = 1.0;

/// A box as drawn: its least and greatest corner, board mm.
type BoxMm = ([f32; 2], [f32; 2]);

/// The panel as the backdrop draws it, board mm.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PanelPreview {
    pub frame: BoxMm,
    pub rails: Vec<BoxMm>,
    /// The outline of each copy but the first, which is the board itself.
    pub copies: Vec<Vec<[f32; 2]>>,
    /// Routed edges and V-score lines.
    pub lines: Vec<([f32; 2], [f32; 2])>,
    /// Fiducials, tooling holes and bite holes: centre and radius.
    pub rings: Vec<([f32; 2], f32)>,
}

fn mm(nm: i64) -> f32 {
    nm_to_mm(nm) as f32
}

fn point(p: pcb_geom::PointNm) -> [f32; 2] {
    [mm(p.x), mm(p.y)]
}

fn rect(r: pcb_geom::RectNm) -> BoxMm {
    (point(r.min), point(r.max))
}

impl PanelPreview {
    /// The preview of `board`'s panel; `None` without a panel or for one
    /// that does not lay out (the Properties panel says why).
    pub fn of(board: &PcbBoard) -> Option<Self> {
        let panel = board.options.panel.as_ref()?;
        let laid = layout(&board.outline, panel).ok()?;
        Some(Self::from_layout(board, &laid))
    }

    fn from_layout(board: &PcbBoard, laid: &PanelLayout) -> Self {
        let outline: Vec<[f32; 2]> = board
            .outline
            .iter()
            .map(|p| [p.x as f32, p.y as f32])
            .collect();
        let copies = laid
            .copies
            .iter()
            .skip(1)
            .map(|offset| {
                let [dx, dy] = point(*offset);
                outline.iter().map(|p| [p[0] + dx, p[1] + dy]).collect()
            })
            .collect();
        let lines = laid
            .routes
            .iter()
            .chain(&laid.v_scores)
            .map(|line| (point(line.from), point(line.to)))
            .collect();
        let ring = |diameter: i64| move |at: &pcb_geom::PointNm| (point(*at), mm(diameter) / 2.0);
        let rings = laid
            .fiducials
            .iter()
            .map(ring(laid.fiducial_diameter))
            .chain(
                laid.tooling_holes
                    .iter()
                    .map(ring(laid.tooling_hole_diameter)),
            )
            .chain(laid.bite_holes.iter().map(ring(laid.bite_diameter)))
            .collect();
        Self {
            frame: rect(laid.frame),
            rails: laid.rails.iter().copied().map(rect).collect(),
            copies,
            lines,
            rings,
        }
    }
}

/// Draw `preview` in the board area's fill and the board edge colour.
pub(super) fn draw_panel(
    frame: &mut Frame,
    camera: &BoardCamera,
    preview: &PanelPreview,
    colors: &CanvasColors,
) {
    let fill = crate::render_config::to_iced(&colors.board_area_color());
    let edge = crate::render_config::to_iced(&colors.board_edge_color());
    let stroke = Stroke::default().with_color(edge).with_width(PANEL_LINE_PX);
    let screen = |p: [f32; 2]| camera.world_to_screen(Point::new(p[0], p[1]));
    let polygon = |points: &[[f32; 2]]| {
        Path::new(|b| {
            if let Some((first, rest)) = points.split_first() {
                b.move_to(screen(*first));
                for p in rest {
                    b.line_to(screen(*p));
                }
                b.close();
            }
        })
    };
    let boxed = |(lo, hi): BoxMm| polygon(&[lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]]);
    for rail in &preview.rails {
        frame.fill(&boxed(*rail), fill);
    }
    for copy in &preview.copies {
        frame.fill(&polygon(copy), fill);
    }
    frame.stroke(&boxed(preview.frame), stroke);
    let mut lines = Builder::new();
    for (a, b) in &preview.lines {
        lines.move_to(screen(*a));
        lines.line_to(screen(*b));
    }
    frame.stroke(&lines.build(), stroke);
    let faint = Color {
        a: edge.a * 0.8,
        ..edge
    };
    for (centre, radius) in &preview.rings {
        let at = screen(*centre);
        let rim = screen([centre[0] + radius, centre[1]]);
        let px = (rim.x - at.x).hypot(rim.y - at.y);
        // Below half a pixel a ring is noise at this zoom.
        if px >= 0.5 {
            frame.stroke(
                &Path::circle(at, px),
                Stroke::default()
                    .with_color(faint)
                    .with_width(PANEL_LINE_PX),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::Point as BoardPoint;
    use signex_types::pcb_panel::BoardPanel;
    use uuid::Uuid;

    #[test]
    fn a_panel_previews_its_frame_rails_copies_and_holes() {
        let mut board: PcbBoard =
            serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
        assert_eq!(PanelPreview::of(&board), None, "no panel, no preview");
        board.outline = vec![
            BoardPoint::new(0.0, 0.0),
            BoardPoint::new(50.0, 0.0),
            BoardPoint::new(50.0, 40.0),
            BoardPoint::new(0.0, 40.0),
        ];
        board.options.panel = Some(BoardPanel::default());
        let preview = PanelPreview::of(&board).expect("a preview");
        assert_eq!(preview.frame, ([0.0, -7.0], [102.0, 89.0]));
        assert_eq!(preview.rails.len(), 2);
        assert_eq!(preview.copies.len(), 3, "the board is the first copy");
        assert_eq!(preview.copies[0][1], [102.0, 0.0]);
        // Three fiducials, four tooling holes and 48 bite holes.
        assert_eq!(preview.rings.len(), 3 + 4 + 48);
        assert!(!preview.lines.is_empty());
    }
}
