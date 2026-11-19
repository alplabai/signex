//! Rooms on the board backdrop (#2457): each room a tinted area in its colour
//! (10 %, 20 % selected), a dashed edge and its name in bold at the top-left
//! in its colour. A selected room's edge takes the board palette's selection
//! colour. Drawn under the copper with the backdrop; a Top room shows on the
//! top-side view, a Bottom room on the bottom side, a Both room always.

use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point};
use signex_types::pcb::{PcbBoard, Room, RoomLayer};
use signex_types::theme::CanvasColors;
use uuid::Uuid;

use super::camera::BoardCamera;

const EDGE_PX: f32 = 1.0;
const SELECTED_EDGE_PX: f32 = 1.5;
/// The edge's dash, px on, px off: the mock's 4 / 2.5.
const DASH: [f32; 2] = [4.0, 2.5];
const FILL_ALPHA: f32 = 0.10;
const SELECTED_FILL_ALPHA: f32 = 0.20;

/// A room as the backdrop draws it, board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomMark {
    pub uuid: Uuid,
    pub corners: Vec<[f32; 2]>,
    pub colour: Color,
    pub name: String,
    pub layer: RoomLayer,
    /// Where its name sits: the top-left of its label box, and the label
    /// height, mm (`pcb_geom::room_label_box`).
    pub label_at: [f32; 2],
    pub label_mm: f32,
}

impl RoomMark {
    /// Every room of `board`; a room whose colour does not read is drawn in
    /// the first palette colour.
    pub fn of(board: &PcbBoard) -> Vec<RoomMark> {
        board.rooms.iter().map(Self::of_room).collect()
    }

    /// One room as drawn.
    pub fn of_room(room: &Room) -> RoomMark {
        let colour = signex_types::theme::Color::parse_hex(&room.colour)
            .or_else(|| {
                signex_types::theme::Color::parse_hex(signex_types::pcb::room::ROOM_PALETTE[0])
            })
            .map(|c| crate::render_config::to_iced(&c))
            .unwrap_or(Color::WHITE);
        let label = pcb_geom::room_label_box(room);
        RoomMark {
            uuid: room.uuid,
            corners: room
                .outline
                .corners()
                .iter()
                .map(|p| [p.x as f32, p.y as f32])
                .collect(),
            colour,
            name: room.name.clone(),
            layer: room.layer,
            label_at: label.map_or([0.0, 0.0], |(lo, _)| [lo.x as f32, lo.y as f32]),
            label_mm: pcb_geom::ROOM_LABEL_HEIGHT_MM as f32,
        }
    }
}

/// What the app adds over the board's rooms: the selected ones, and the
/// room Place ▸ Room is drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomsOverlay {
    pub selected: Vec<Uuid>,
    pub drawing: Option<RoomMark>,
}

/// Draw `rooms`, and the one being drawn, for the side `camera` shows; the
/// selected ones stand out.
pub(super) fn draw_rooms(
    frame: &mut Frame,
    camera: &BoardCamera,
    rooms: &[RoomMark],
    overlay: &RoomsOverlay,
    colors: &CanvasColors,
) {
    let selected: Vec<Uuid> = overlay
        .selected
        .iter()
        .copied()
        .chain(overlay.drawing.iter().map(|r| r.uuid))
        .collect();
    let rooms = rooms.iter().chain(overlay.drawing.iter());
    let bottom_view = camera.view_flipped();
    let selection = crate::render_config::to_iced(&colors.selection);
    let screen = |p: [f32; 2]| camera.world_to_screen(Point::new(p[0], p[1]));
    let font = iced::Font {
        weight: iced::font::Weight::Bold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    for room in rooms.filter(|r| r.layer.shows_on(bottom_view)) {
        let is_selected = selected.contains(&room.uuid);
        let path = Path::new(|b| {
            if let Some((first, rest)) = room.corners.split_first() {
                b.move_to(screen(*first));
                for p in rest {
                    b.line_to(screen(*p));
                }
                b.close();
            }
        });
        let alpha = if is_selected {
            SELECTED_FILL_ALPHA
        } else {
            FILL_ALPHA
        };
        frame.fill(
            &path,
            Color {
                a: alpha,
                ..room.colour
            },
        );
        let (edge, width) = if is_selected {
            (selection, SELECTED_EDGE_PX)
        } else {
            (room.colour, EDGE_PX)
        };
        frame.stroke(
            &path,
            Stroke {
                line_dash: LineDash {
                    segments: &DASH,
                    offset: 0,
                },
                ..Stroke::default().with_width(width).with_color(edge)
            },
        );
        let at = screen(room.label_at);
        let below = screen([room.label_at[0], room.label_at[1] + room.label_mm]);
        let size = (below.y - at.y).abs().max((below.x - at.x).abs());
        // Below a few pixels a name is noise at this zoom.
        if size >= 4.0 {
            frame.fill_text(Text {
                content: room.name.clone(),
                position: at,
                color: room.colour,
                size: Pixels(size),
                font,
                ..Text::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{Point as BoardPoint, Room, RoomOutline};

    #[test]
    fn a_room_is_marked_with_its_corners_colour_and_label() {
        let mut board: PcbBoard =
            serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
        board.rooms.push(Room::drawn(
            Uuid::from_u128(1),
            "dcdc".into(),
            "#A058C8",
            RoomOutline::rect_between(BoardPoint::new(20.0, 2.0), BoardPoint::new(44.0, 35.0)),
        ));
        let marks = RoomMark::of(&board);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].corners[0], [20.0, 2.0]);
        assert_eq!(marks[0].corners[2], [44.0, 35.0]);
        assert_eq!(marks[0].name, "dcdc");
        let inset = pcb_geom::ROOM_LABEL_INSET_MM;
        assert_eq!(
            marks[0].label_at,
            [(20.0 + inset) as f32, (2.0 + inset) as f32]
        );
        let purple = signex_types::theme::Color::parse_hex("#A058C8").unwrap();
        assert_eq!(marks[0].colour, crate::render_config::to_iced(&purple));
    }
}
