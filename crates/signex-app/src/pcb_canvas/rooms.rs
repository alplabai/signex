//! Rooms on the board backdrop (#2457): each room a tinted area in its colour
//! (10 %, 20 % selected), a dashed edge and its name in bold at the top-left
//! in its colour, 12 to 16 px on screen. A selected room's edge takes the board palette's selection
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
/// The room name's size on screen, px: the mockup's 12 px at fit zoom, and
/// the most it grows to when zoomed in.
const NAME_MIN_PX: f32 = 12.0;
const NAME_MAX_PX: f32 = 16.0;
/// A room shows its name while its smaller side on screen is at least
/// this many name heights.
const NAME_ROOM_RATIO: f32 = 2.0;

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
    let bottom_view = camera.rotation().is_mirrored();
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
        let natural = (below.y - at.y).abs().max((below.x - at.x).abs());
        let room_px = screen_extent(room.corners.iter().map(|p| screen(*p)));
        if let Some(size) = name_px(natural, room_px) {
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

/// The smaller side of the screen box around `points`, px.
fn screen_extent(points: impl Iterator<Item = Point>) -> f32 {
    let (mut lo, mut hi) = (
        Point::new(f32::INFINITY, f32::INFINITY),
        Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    );
    for p in points {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (hi.x - lo.x).min(hi.y - lo.y).max(0.0)
}

/// The pixel size a room's name is drawn at: its board size at this zoom,
/// kept between [`NAME_MIN_PX`] and [`NAME_MAX_PX`], so it reads at fit
/// zoom and stops growing when zoomed in (mockup row 163, #3061). None
/// when the room on screen is too small to hold it: zoomed far out.
fn name_px(natural_px: f32, room_px: f32) -> Option<f32> {
    let size = natural_px.clamp(NAME_MIN_PX, NAME_MAX_PX);
    (room_px >= size * NAME_ROOM_RATIO).then_some(size)
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

    #[test]
    fn the_name_size_is_kept_between_the_floor_and_the_cap() {
        assert_eq!(name_px(6.0, 132.0), Some(NAME_MIN_PX));
        assert_eq!(name_px(14.0, 132.0), Some(14.0));
        assert_eq!(name_px(60.0, 1300.0), Some(NAME_MAX_PX));
        // A room smaller on screen than two name heights draws no name.
        assert_eq!(name_px(2.0, 20.0), None);
    }

    /// Draws `rooms` the way the board backdrop does, through `camera`.
    struct Backdrop {
        camera: BoardCamera,
        rooms: Vec<RoomMark>,
    }

    impl iced::widget::canvas::Program<()> for Backdrop {
        type State = ();

        fn draw(
            &self,
            _state: &(),
            renderer: &iced::Renderer,
            _theme: &iced::Theme,
            bounds: iced::Rectangle,
            _cursor: iced::mouse::Cursor,
        ) -> Vec<iced::widget::canvas::Geometry> {
            let mut frame = Frame::new(renderer, bounds.size());
            let colors = crate::styles::board_canvas_colors();
            draw_rooms(
                &mut frame,
                &self.camera,
                &self.rooms,
                &RoomsOverlay::default(),
                &colors,
            );
            vec![frame.into_geometry()]
        }
    }

    /// How many pixel rows the room's name covers: rows inside the room's
    /// top-left corner (clear of its dashed edge) holding a pixel in the
    /// room's colour, which the 10 % fill is far from.
    fn name_rows(camera: &BoardCamera, room: &RoomMark, window: iced::Size) -> usize {
        use iced::Widget as _;
        use iced::widget::canvas;
        let element: iced::Element<'_, ()> = canvas(Backdrop {
            camera: camera.clone(),
            rooms: vec![room.clone()],
        })
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .boxed();
        let pixels = crate::layout_probe::draw_pixels(element, window);
        let corner = camera.world_to_screen(Point::new(room.corners[0][0], room.corners[0][1]));
        let (w, h) = (window.width as usize, window.height as usize);
        let x0 = (corner.x.max(0.0) as usize + 3).min(w);
        let y0 = (corner.y.max(0.0) as usize + 3).min(h);
        let channel = |v: f32| (v * 255.0).round() as i32;
        let want = [
            channel(room.colour.r),
            channel(room.colour.g),
            channel(room.colour.b),
        ];
        (y0..(y0 + 80).min(h))
            .filter(|&y| {
                (x0..(x0 + 100).min(w)).any(|x| {
                    let at = (y * w + x) * 4;
                    (0..3).all(|c| (i32::from(pixels[at + c]) - want[c]).abs() <= 48)
                })
            })
            .count()
    }

    /// Mockup row 163 (#3061): at fit zoom the room's name is drawn at the
    /// readable floor, not its 1 mm board size (6 px here), and zoomed in
    /// it stops at the cap instead of growing with the board (60 px here).
    /// The board canvas is dark in both app themes, so one probe covers
    /// Light and Dark.
    #[test]
    fn the_room_name_reads_at_fit_zoom_and_stops_growing_zoomed_in() {
        let room = RoomMark::of_room(&Room::drawn(
            Uuid::from_u128(1),
            "dcdc".into(),
            "#A058C8",
            RoomOutline::rect_between(BoardPoint::new(20.0, 2.0), BoardPoint::new(42.0, 22.0)),
        ));
        let window = iced::Size::new(480.0, 320.0);
        let mut camera = BoardCamera::default();
        camera.fit_rect(
            iced::Rectangle::new(Point::ORIGIN, iced::Size::new(60.0, 40.0)),
            iced::Rectangle::with_size(window),
        );
        assert!(camera.scale() < 8.0, "fit zoom: {} px/mm", camera.scale());
        let fit = name_rows(&camera, &room, window);
        // A bold "dcdc" at 12 px is about 9 rows tall; at 6 px about 4.
        assert!(fit >= 7, "name {fit} rows tall at fit zoom");
        let corner = camera.world_to_screen(Point::new(20.0, 2.0));
        camera.zoom_by_factor(corner, 10.0);
        let zoomed = name_rows(&camera, &room, window);
        assert!(
            (7..=NAME_MAX_PX as usize).contains(&zoomed),
            "name {zoomed} rows tall at {} px/mm",
            camera.scale()
        );
    }
}
