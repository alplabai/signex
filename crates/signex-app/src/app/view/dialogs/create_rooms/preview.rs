//! The Create Rooms from Sheets preview (#2457): the board outline on the
//! board palette's dark ground (the same in both themes), its parts, the
//! planned rooms dashed in their colours, and the parts Update PCB left below
//! the board drawn dashed where they are now.

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, LineDash, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget as _};
use signex_engine::pcb::rooms::{RoomPlan, part_box};
use signex_types::pcb::PcbBoard;

use crate::app::Message;

const DASH: [f32; 2] = [4.0, 2.5];
const STAGED_DASH: [f32; 2] = [1.5, 1.0];
/// Pixels left round the drawing.
const INSET: f32 = 12.0;

/// What the preview draws, board mm.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Preview {
    outline: Vec<[f32; 2]>,
    /// Part bodies: least and greatest corner, and whether the plan moves it.
    parts: Vec<([f32; 4], bool)>,
    rooms: Vec<(Vec<[f32; 2]>, Color)>,
    ground: Color,
    area: Color,
    edge: Color,
    ink: Color,
}

impl Preview {
    pub(super) fn of(board: &PcbBoard, plan: &RoomPlan) -> Self {
        let colors = crate::styles::board_canvas_colors();
        let to = |c: signex_types::theme::Color| crate::render_config::to_iced(&c);
        let parts = board
            .footprints
            .iter()
            .map(|fp| {
                let b = part_box(fp);
                let moved = plan.moves.iter().any(|(id, _)| *id == fp.uuid);
                (
                    [
                        b.min.x as f32,
                        b.min.y as f32,
                        b.max.x as f32,
                        b.max.y as f32,
                    ],
                    moved,
                )
            })
            .collect();
        let rooms = plan
            .rooms
            .iter()
            .map(|p| {
                let corners = p
                    .room
                    .outline
                    .corners()
                    .iter()
                    .map(|c| [c.x as f32, c.y as f32])
                    .collect();
                (
                    corners,
                    crate::panels::pcb_room_properties::room_colour(&p.room.colour),
                )
            })
            .collect();
        Self {
            outline: board
                .outline
                .iter()
                .map(|p| [p.x as f32, p.y as f32])
                .collect(),
            parts,
            rooms,
            ground: to(colors.background),
            area: to(colors.board_area_color()),
            edge: to(colors.board_edge_color()),
            ink: to(colors.body),
        }
    }

    /// The box round everything drawn.
    fn extent(&self) -> Option<([f32; 2], [f32; 2])> {
        let points = self
            .outline
            .iter()
            .copied()
            .chain(
                self.parts
                    .iter()
                    .flat_map(|(b, _)| [[b[0], b[1]], [b[2], b[3]]]),
            )
            .chain(self.rooms.iter().flat_map(|(r, _)| r.iter().copied()));
        points.fold(None, |acc, p| {
            Some(match acc {
                None => (p, p),
                Some((lo, hi)) => (
                    [lo[0].min(p[0]), lo[1].min(p[1])],
                    [hi[0].max(p[0]), hi[1].max(p[1])],
                ),
            })
        })
    }

    pub(super) fn view<'a>(self) -> Element<'a, Message> {
        canvas::Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }
}

impl canvas::Program<Message> for Preview {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.ground);
        let Some((lo, hi)) = self.extent() else {
            return vec![frame.into_geometry()];
        };
        let (w, h) = ((hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0));
        let scale = ((bounds.width - 2.0 * INSET) / w).min((bounds.height - 2.0 * INSET) / h);
        let at = |p: [f32; 2]| {
            Point::new(
                INSET + (p[0] - lo[0]) * scale,
                INSET + (p[1] - lo[1]) * scale,
            )
        };
        let ring = |points: &[[f32; 2]]| {
            Path::new(|b| {
                if let Some((first, rest)) = points.split_first() {
                    b.move_to(at(*first));
                    for p in rest {
                        b.line_to(at(*p));
                    }
                    b.close();
                }
            })
        };
        let dashed = |color: Color, dash: &'static [f32]| Stroke {
            line_dash: LineDash {
                segments: dash,
                offset: 0,
            },
            ..Stroke::default().with_width(1.0).with_color(color)
        };
        let outline = ring(&self.outline);
        frame.fill(&outline, self.area);
        frame.stroke(
            &outline,
            Stroke::default().with_width(1.0).with_color(self.edge),
        );
        for (corners, colour) in &self.rooms {
            let path = ring(corners);
            frame.fill(&path, Color { a: 0.10, ..*colour });
            frame.stroke(&path, dashed(*colour, &DASH));
        }
        for (b, moved) in &self.parts {
            let min = at([b[0], b[1]]);
            let max = at([b[2], b[3]]);
            let size = Size::new((max.x - min.x).max(2.0), (max.y - min.y).max(2.0));
            let path = Path::rectangle(min, size);
            if *moved {
                frame.stroke(&path, dashed(self.ink, &STAGED_DASH));
            } else {
                frame.stroke(
                    &path,
                    Stroke::default().with_width(1.0).with_color(self.ink),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}
