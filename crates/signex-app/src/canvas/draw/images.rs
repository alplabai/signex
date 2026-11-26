//! The pictures of placed images (#1116, mockup row 204). An image behind
//! the circuit draws above the paper and under the content, one above the
//! circuit over the content. The picture comes from the app's image cache,
//! read in a `Task`; a missing file draws a hatched placeholder with its
//! path in the warning ink. Drawn every frame: the renderer keeps the
//! decoded picture by its handle, so a pan or zoom costs no decode.

use super::super::*;
use crate::app::images::{ImageEntry, ImageKey};
use signex_types::schematic::{DrawingPlacement, SchDrawing, SchImage};

/// Spacing of the placeholder's hatch lines, in screen pixels.
const HATCH_SPACING_PX: f32 = 8.0;
/// Width of the placeholder's dashed outline, in screen pixels.
const PLACEHOLDER_OUTLINE_PX: f32 = 1.5;
/// Dash and gap of the placeholder's outline, in screen pixels.
const PLACEHOLDER_DASH_PX: [f32; 2] = [6.0, 4.0];
/// Size of the placeholder's path text, in screen pixels.
const PLACEHOLDER_TEXT_PX: f32 = 11.0;
/// Alphas of the placeholder's wash and hatch over the warning ink.
const PLACEHOLDER_WASH_ALPHA: f32 = 0.08;
const PLACEHOLDER_HATCH_ALPHA: f32 = 0.35;

impl SchematicCanvas<'_> {
    /// The pictures of `sheet`'s images placed at `placement`; `None` when
    /// it has none.
    pub(in crate::canvas) fn draw_images(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        sheet: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        placement: DrawingPlacement,
    ) -> Option<canvas::Geometry> {
        let sheet = sheet?;
        let mut images = sheet
            .drawings
            .iter()
            .filter_map(|d| match d {
                SchDrawing::Image(image) if sheet.drawing_placement(image.uuid) == placement => {
                    Some(image)
                }
                _ => None,
            })
            .peekable();
        images.peek()?;
        let cam = self.camera();
        let view = Rectangle::with_size(bounds.size());
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        for image in images {
            let (lo, hi) = image.bounds();
            let a = cam.world_to_screen(iced::Point::new(lo.x as f32, lo.y as f32), bounds);
            let b = cam.world_to_screen(iced::Point::new(hi.x as f32, hi.y as f32), bounds);
            let rect = Rectangle::new(
                iced::Point::new(a.x.min(b.x), a.y.min(b.y)),
                iced::Size::new((b.x - a.x).abs(), (b.y - a.y).abs()),
            );
            if !rect.intersects(&view) {
                continue;
            }
            let entry = self
                .prefs
                .image_cache
                .and_then(|cache| cache.entry(&ImageKey::of(image)));
            match entry {
                Some(ImageEntry::Raster(handle)) => frame.draw_image(
                    rect,
                    canvas::Image::new(handle.clone()).opacity(image.alpha()),
                ),
                Some(ImageEntry::Vector(handle)) => frame.draw_svg(
                    rect,
                    iced::advanced::svg::Svg::new(handle.clone()).opacity(image.alpha()),
                ),
                Some(ImageEntry::Missing) => self.draw_image_placeholder(&mut frame, rect, image),
                Some(ImageEntry::Loading) | None => {}
            }
        }
        Some(frame.into_geometry())
    }

    /// A missing image: its box washed and hatched in the warning ink, a
    /// dashed outline, and its path when the box has room for it.
    fn draw_image_placeholder(&self, frame: &mut canvas::Frame, rect: Rectangle, image: &SchImage) {
        let ink = self.prefs.graphite.warn;
        frame.fill_rectangle(
            rect.position(),
            rect.size(),
            Color {
                a: PLACEHOLDER_WASH_ALPHA,
                ..ink
            },
        );
        let (w, h) = (rect.width, rect.height);
        let hatch = canvas::Path::new(|path| {
            // Lines along x + y = t, from the top or right edge to the left
            // or bottom edge.
            let mut t = HATCH_SPACING_PX;
            while t < w + h {
                let start = if t <= w {
                    iced::Point::new(rect.x + t, rect.y)
                } else {
                    iced::Point::new(rect.x + w, rect.y + (t - w))
                };
                let end = if t <= h {
                    iced::Point::new(rect.x, rect.y + t)
                } else {
                    iced::Point::new(rect.x + (t - h), rect.y + h)
                };
                path.move_to(start);
                path.line_to(end);
                t += HATCH_SPACING_PX;
            }
        });
        frame.stroke(
            &hatch,
            canvas::Stroke::default().with_color(Color {
                a: PLACEHOLDER_HATCH_ALPHA,
                ..ink
            }),
        );
        let mut outline = canvas::Stroke::default()
            .with_color(ink)
            .with_width(PLACEHOLDER_OUTLINE_PX);
        outline.line_dash = canvas::LineDash {
            segments: &PLACEHOLDER_DASH_PX,
            offset: 0,
        };
        frame.stroke(
            &canvas::Path::rectangle(rect.position(), rect.size()),
            outline,
        );
        if h > PLACEHOLDER_TEXT_PX * 2.0 {
            frame.fill_text(canvas::Text {
                content: image.file.clone(),
                position: rect.center(),
                max_width: w,
                color: ink,
                size: iced::Pixels(PLACEHOLDER_TEXT_PX),
                font: crate::render_config::IOSEVKA,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
    }
}
