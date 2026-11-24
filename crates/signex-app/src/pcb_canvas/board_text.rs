//! Footprint designator and comment texts on the board (#2368). Rules:
//! `docs/RENDERING_RULES.md`, "pcb-footprint-text".
//!
//! A canvas layer of its own, above the board content on both mounts, so
//! the CPU and GPU mounts draw the same text and a bottom-side text can be
//! mirrored, which the GPU glyph atlas cannot do.

use iced::advanced::text as advanced_text;
use iced::widget::canvas;
use iced::{Color, Point, Radians, Rectangle, Renderer, Size, Theme, Vector, alignment, mouse};
use signex_gfx::primitive::text::{
    LINE_HEIGHT_EM, TextRaster, TextSizePolicy, text_px, text_raster,
};
use signex_renderer::pcb::{BoardPaint, DIMMED_ALPHA, FootprintTextInput};
use signex_types::pcb_layer::PcbLayer;
use signex_types::schematic::HAlign;

use super::layer_style::painted_layer_colour;
use super::{BoardCamera, PcbCanvas};
use crate::app::Message;
use crate::renderer_scene_canvas::ATLAS_TEXT_LIMIT_PX;

/// What the retained text was drawn at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TextKey {
    offset: (f32, f32),
    scale: f32,
    degrees: f32,
    mirrored: bool,
    scene_generation: u64,
}

impl TextKey {
    fn live(canvas: &PcbCanvas, camera: &BoardCamera) -> Self {
        Self {
            offset: (camera.offset().x, camera.offset().y),
            scale: camera.scale(),
            degrees: camera.rotation().degrees(),
            mirrored: camera.rotation().is_mirrored(),
            scene_generation: canvas.scene_generation(),
        }
    }
}

/// Paints [`PcbCanvas`]'s footprint texts; takes no input.
pub struct BoardTextLayer<'a>(pub(super) &'a PcbCanvas);

impl PcbCanvas {
    pub fn text_layer(&self) -> BoardTextLayer<'_> {
        BoardTextLayer(self)
    }
}

impl canvas::Program<Message> for BoardTextLayer<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let canvas = self.0;
        let Some(snapshot) = canvas.renderer_snapshot.as_ref() else {
            return Vec::new();
        };
        if snapshot.texts.is_empty() {
            return Vec::new();
        }
        let camera = canvas.camera_ref();
        let key = TextKey::live(canvas, &camera);
        if canvas.text_key.get() != Some(key) {
            canvas.text_cache.clear();
            canvas.text_key.set(Some(key));
        }
        let view = Rectangle::with_size(bounds.size());
        let geometry = canvas.text_cache.draw(renderer, bounds.size(), |frame| {
            paint_texts(frame, &snapshot.texts, &snapshot.paint, &camera, view);
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

/// Paint `texts` at `camera` onto `frame`, leaving out what cannot reach
/// `view`. Each text is justified on its anchor in its own frame, turned by
/// its angle and the view's turn, and mirrored on the bottom side, in its
/// layer's colour in `paint` (#2363).
pub(super) fn paint_texts(
    frame: &mut canvas::Frame,
    texts: &[FootprintTextInput],
    paint: &BoardPaint,
    camera: &BoardCamera,
    view: Rectangle,
) {
    let mut colours: Vec<(PcbLayer, Color)> = Vec::new();
    let turn = camera.rotation().degrees();
    // Seen from below (#2394) a text turns the other way and its mirror
    // flips, as its board geometry does.
    let from_below = camera.rotation().is_mirrored();
    for text in texts {
        let Some(placed) = place(text, camera, view) else {
            continue;
        };
        let color = colour_of(&mut colours, paint, text.layer);
        let glyphs = canvas::Text {
            content: text.content.clone(),
            position: Point::ORIGIN,
            color,
            size: iced::Pixels(placed.size_px),
            line_height: iced::widget::text::LineHeight::Relative(LINE_HEIGHT_EM),
            font: crate::render_config::IOSEVKA,
            align_x: align_of(text.justify),
            align_y: alignment::Vertical::Center,
            ..canvas::Text::default()
        };
        let (rotation, mirrored) = if from_below {
            (-text.rotation_deg, !text.mirrored)
        } else {
            (text.rotation_deg, text.mirrored)
        };
        let angle = (rotation + turn).rem_euclid(360.0);
        frame.with_save(|inner| {
            inner.translate(Vector::new(placed.anchor.x, placed.anchor.y));
            if angle != 0.0 {
                inner.rotate(Radians(angle.to_radians()));
            }
            if mirrored {
                inner.scale_nonuniform(Vector::new(-1.0, 1.0));
            }
            inner.fill_text(glyphs);
        });
    }
}

/// A text's anchor on screen and its em in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ScreenText {
    pub(super) anchor: Point,
    pub(super) size_px: f32,
}

/// Where `text` lands on screen at `camera`: `None` when it is too small or
/// too large to draw, or cannot reach `view`. True size: board text is
/// geometry, so it is not held to a readable size when zoomed out.
pub(super) fn place(
    text: &FootprintTextInput,
    camera: &BoardCamera,
    view: Rectangle,
) -> Option<ScreenText> {
    let size_px = text_px(text.size_mm, camera.scale(), TextSizePolicy::UNCLAMPED);
    if text_raster(size_px, ATLAS_TEXT_LIMIT_PX) == TextRaster::Skip {
        return None;
    }
    let anchor = camera.world_to_screen(Point::new(text.position[0], text.position[1]));
    // Generous: a monospace em is wider than its glyph advance.
    let reach = size_px * (text.content.chars().count() as f32 + 1.0);
    let extent = Rectangle::new(
        Point::new(anchor.x - reach, anchor.y - reach),
        Size::new(2.0 * reach, 2.0 * reach),
    );
    view.intersects(&extent)
        .then_some(ScreenText { anchor, size_px })
}

/// The anchor end of a text's line: applied before the mirror, so a
/// mirrored text runs the other way from its anchor, as its strokes do.
pub(super) fn align_of(justify: HAlign) -> advanced_text::Alignment {
    match justify {
        HAlign::Left => advanced_text::Alignment::Left,
        HAlign::Center => advanced_text::Alignment::Center,
        HAlign::Right => advanced_text::Alignment::Right,
    }
}

fn colour_of(colours: &mut Vec<(PcbLayer, Color)>, paint: &BoardPaint, layer: PcbLayer) -> Color {
    if let Some(&(_, colour)) = colours.iter().find(|(l, _)| *l == layer) {
        return colour;
    }
    let colour = text_colour(paint, layer);
    colours.push((layer, colour));
    colour
}

/// A text's colour on `layer`, faint under Show only unrouted (#2821).
fn text_colour(paint: &BoardPaint, layer: PcbLayer) -> Color {
    let colour = painted_layer_colour(paint, layer);
    if paint.is_board_dimmed() {
        Color {
            a: colour.a * DIMMED_ALPHA,
            ..colour
        }
    } else {
        colour
    }
}

#[cfg(test)]
#[path = "board_text_tests.rs"]
mod tests;
