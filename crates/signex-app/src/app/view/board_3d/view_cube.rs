//! The view cube at the 3D view's top right (m175): three faces of a cube
//! seen from above the front-left corner, named TOP, LEFT and FRONT, or
//! BOTTOM, RIGHT and FRONT while the board is turned over.

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Point, Rectangle, Renderer, Theme, Widget as _, mouse};
use signex_gfx::camera3d::ViewSide;
use signex_i18n::{I18n, ids, t};

use super::{Look, M, TS};
use crate::app::Message;

/// The cube's three faces in the mockup's 64-unit box: top, left, right.
const TOP_FACE: [[f32; 2]; 4] = [[32.0, 6.0], [56.0, 19.0], [32.0, 32.0], [8.0, 19.0]];
const LEFT_FACE: [[f32; 2]; 4] = [[8.0, 19.0], [32.0, 32.0], [32.0, 58.0], [8.0, 45.0]];
const RIGHT_FACE: [[f32; 2]; 4] = [[56.0, 19.0], [32.0, 32.0], [32.0, 58.0], [56.0, 45.0]];
/// Where each face's name sits, in the same box.
const TOP_LABEL: [f32; 2] = [32.0, 19.0];
const LEFT_LABEL: [f32; 2] = [20.0, 39.0];
const RIGHT_LABEL: [f32; 2] = [44.0, 39.0];
/// The mockup box the coordinates above are drawn in.
const BOX: f32 = 64.0;

/// The cube: its face fills, line and ink, and its three names.
struct ViewCube {
    fills: [Color; 3],
    line: Color,
    ink: Color,
    names: [String; 3],
}

/// The view cube for the side shown.
pub(super) fn view_cube<'a>(i18n: &I18n, side: ViewSide, look: &Look) -> Element<'a, Message> {
    let (top, left, right) = match side {
        ViewSide::Top => (
            ids::PCB_3D_CUBE_TOP,
            ids::PCB_3D_CUBE_LEFT,
            ids::PCB_3D_CUBE_FRONT,
        ),
        ViewSide::Bottom => (
            ids::PCB_3D_CUBE_BOTTOM,
            ids::PCB_3D_CUBE_RIGHT,
            ids::PCB_3D_CUBE_FRONT,
        ),
    };
    let cube = ViewCube {
        fills: [look.r.raised, look.r.sunken, look.hover_fill],
        line: look.r.line_strong,
        ink: look.ink2,
        names: [
            t!(i18n, top).into_string(),
            t!(i18n, left).into_string(),
            t!(i18n, right).into_string(),
        ],
    };
    iced::widget::canvas(cube)
        .width(M.cube)
        .height(M.cube)
        .boxed()
}

impl canvas::Program<Message> for ViewCube {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let scale = bounds.width.min(bounds.height) / BOX;
        let at = |[x, y]: [f32; 2]| Point::new(x * scale, y * scale);
        let faces = [TOP_FACE, LEFT_FACE, RIGHT_FACE];
        for (face, fill) in faces.iter().zip(self.fills) {
            let path = Path::new(|p| {
                p.move_to(at(face[0]));
                for corner in &face[1..] {
                    p.line_to(at(*corner));
                }
                p.close();
            });
            frame.fill(&path, fill);
            frame.stroke(
                &path,
                Stroke::default().with_color(self.line).with_width(1.0),
            );
        }
        // The mockup sets the top name at 9 px and the side names at 7 px.
        let sizes = [TS.caption * 9.0 / 10.0, TS.caption * 7.0 / 10.0];
        let spots = [TOP_LABEL, LEFT_LABEL, RIGHT_LABEL];
        for (index, (name, spot)) in self.names.iter().zip(spots).enumerate() {
            let size = if index == 0 { sizes[0] } else { sizes[1] };
            frame.fill_text(Text {
                content: name.clone(),
                position: at(spot),
                color: self.ink,
                size: iced::Pixels(size * scale),
                align_x: iced::advanced::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                font: iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::DEFAULT
                },
                ..Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}
