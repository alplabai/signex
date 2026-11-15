//! What the board backdrop draws from the board options (#2374): the
//! sheet behind the board area, the extra snap grids over it and the
//! enabled guides across the view. Pure draw helpers; the canvas holds a
//! [`BoardMarks`] copied from the board when it changes.

use iced::widget::canvas::path::Builder;
use iced::widget::canvas::{Frame, Stroke};
use iced::{Color, Point, Size};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_options::GuideKind;

use super::camera::BoardCamera;
use super::grid;

const MARK_LINE_PX: f32 = 1.0;

/// A guide as drawn: its direction, a point on it and its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideMark {
    pub kind: GuideKind,
    pub at: [f32; 2],
    pub color: Color,
}

/// An extra grid as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMark {
    pub step_mm: f32,
    pub origin: [f32; 2],
    pub color: Color,
}

/// The board options the backdrop draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardMarks {
    /// Enabled guides only.
    pub guides: Vec<GuideMark>,
    /// Extra grids, lowest priority first so the first one draws on top.
    pub grids: Vec<GridMark>,
    /// The stored sheet box (min, max) in board mm; `None` when the sheet
    /// is the box around the board shape.
    pub sheet: Option<([f32; 2], [f32; 2])>,
}

impl BoardMarks {
    pub fn of(board: &PcbBoard) -> Self {
        let options = &board.options;
        let color = |c| crate::render_config::to_iced(&c);
        let guides = options
            .guides
            .iter()
            .filter(|g| g.enabled)
            .map(|g| GuideMark {
                kind: g.kind,
                at: [g.at.x as f32, g.at.y as f32],
                color: color(g.color),
            })
            .collect();
        let grids = options
            .grids
            .iter()
            .rev()
            .map(|g| GridMark {
                step_mm: g.step_mm as f32,
                origin: [g.origin.x as f32, g.origin.y as f32],
                color: color(g.color),
            })
            .collect();
        let s = &options.sheet;
        let sheet = (!s.from_board).then(|| {
            let lo = [s.origin.x as f32, s.origin.y as f32];
            (
                [lo[0], lo[1]],
                [lo[0] + s.width as f32, lo[1] + s.height as f32],
            )
        });
        Self {
            guides,
            grids,
            sheet,
        }
    }
}

/// The sheet box: the stored one, else the box around `region`.
pub(super) fn sheet_box(marks: &BoardMarks, region: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    if let Some(sheet) = marks.sheet {
        return sheet;
    }
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in region {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    (lo, hi)
}

/// Fill the sheet box with `fill` and outline it in `edge`.
pub(super) fn draw_sheet(
    frame: &mut Frame,
    camera: &BoardCamera,
    sheet: ([f32; 2], [f32; 2]),
    fill: Color,
    edge: Color,
) {
    let (lo, hi) = sheet;
    if !(lo[0] < hi[0] && lo[1] < hi[1]) {
        return;
    }
    let corners = [
        [lo[0], lo[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
        [lo[0], hi[1]],
    ];
    let mut b = Builder::new();
    b.move_to(camera.world_to_screen(Point::new(corners[0][0], corners[0][1])));
    for c in &corners[1..] {
        b.line_to(camera.world_to_screen(Point::new(c[0], c[1])));
    }
    b.close();
    let path = b.build();
    frame.fill(&path, fill);
    frame.stroke(
        &path,
        Stroke::default().with_color(edge).with_width(MARK_LINE_PX),
    );
}

/// Stroke each extra grid inside `region` in its own colour, at the level
/// and fade the board grid uses at this zoom.
pub(super) fn draw_grids(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    region: &[[f32; 2]],
    grids: &[GridMark],
) {
    let Some(view) = camera.visible_world_bounds(size) else {
        return;
    };
    let mut scratch = Vec::with_capacity(8);
    for mark in grids {
        let Some(level) = grid::grid_level(mark.step_mm, camera.scale()) else {
            continue;
        };
        // Lines sit on multiples of the step from the grid's own origin.
        let [ox, oy] = mark.origin;
        let shifted: Vec<[f32; 2]> = region.iter().map(|p| [p[0] - ox, p[1] - oy]).collect();
        let view = (
            [view.0[0] - ox, view.0[1] - oy],
            [view.1[0] - ox, view.1[1] - oy],
        );
        let mut path = Builder::new();
        grid::each_segment(&shifted, view, level.pitch_mm, &mut scratch, |_, a, b| {
            path.move_to(camera.world_to_screen(Point::new(a.x + ox, a.y + oy)));
            path.line_to(camera.world_to_screen(Point::new(b.x + ox, b.y + oy)));
        });
        let color = Color {
            a: mark.color.a * level.fade.max(0.35),
            ..mark.color
        };
        frame.stroke(
            &path.build(),
            Stroke::default().with_color(color).with_width(MARK_LINE_PX),
        );
    }
}

/// Stroke each guide across the whole view.
pub(super) fn draw_guides(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    guides: &[GuideMark],
) {
    let Some((lo, hi)) = camera.visible_world_bounds(size) else {
        return;
    };
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let diagonal = (hi[0] - lo[0]).hypot(hi[1] - lo[1]);
    for guide in guides {
        let (dx, dy) = guide.kind.direction();
        let (dx, dy) = (dx as f32, dy as f32);
        let [ax, ay] = guide.at;
        // Long enough to cross the view from any point on the line.
        let reach = (ax - center[0]).hypot(ay - center[1]) + diagonal;
        let mut path = Builder::new();
        path.move_to(camera.world_to_screen(Point::new(ax - dx * reach, ay - dy * reach)));
        path.line_to(camera.world_to_screen(Point::new(ax + dx * reach, ay + dy * reach)));
        frame.stroke(
            &path.build(),
            Stroke::default()
                .with_color(guide.color)
                .with_width(MARK_LINE_PX),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::Point as BoardPoint;
    use signex_types::pcb_options::{BoardGuide, BoardSheet};
    use uuid::Uuid;

    #[test]
    fn marks_keep_enabled_guides_and_a_stored_sheet() {
        let mut board: PcbBoard =
            serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
        let mut off = BoardGuide::new(Uuid::from_u128(1), GuideKind::Vertical, BoardPoint::ZERO);
        off.enabled = false;
        let on = BoardGuide::new(
            Uuid::from_u128(2),
            GuideKind::Horizontal,
            BoardPoint::new(0.0, 4.0),
        );
        board.options.guides = vec![off, on];
        let marks = BoardMarks::of(&board);
        assert_eq!(marks.guides.len(), 1);
        assert_eq!(marks.guides[0].at, [0.0, 4.0]);
        assert_eq!(marks.sheet, None, "the sheet follows the board shape");
        let region = [[1.0, 2.0], [11.0, 2.0], [11.0, 9.0]];
        assert_eq!(sheet_box(&marks, &region), ([1.0, 2.0], [11.0, 9.0]));

        board.options.sheet = BoardSheet {
            from_board: false,
            origin: BoardPoint::new(-5.0, -5.0),
            width: 20.0,
            height: 10.0,
        };
        let marks = BoardMarks::of(&board);
        assert_eq!(marks.sheet, Some(([-5.0, -5.0], [15.0, 5.0])));
    }
}
