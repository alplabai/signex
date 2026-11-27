//! The handles of a selected image and the drag in flight (#1116, mockup row
//! 204): eight hollow squares on the box; while a handle is held, the box
//! the drop would give, its held handle filled, and a pill with the size and
//! the scale. Drawn on the overlay each frame.

use super::super::image_grip::{ImageDrag, selected_images};
use super::super::*;
use signex_types::schematic::{ImageHandle, Point as SheetPoint};

/// Half the side of a handle, screen px.
const HANDLE_HALF_PX: f32 = 4.0;
/// The handle outline and the dragged box, screen px.
const HANDLE_LINE_PX: f32 = 1.2;

impl SchematicCanvas<'_> {
    fn image_box_on_screen(
        &self,
        position: SheetPoint,
        size: (f64, f64),
        bounds: Rectangle,
    ) -> (iced::Point, iced::Point) {
        let cam = self.camera();
        let to =
            |p: SheetPoint| cam.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
        (
            to(position),
            to(SheetPoint::new(position.x + size.0, position.y + size.1)),
        )
    }

    /// The eight squares of a box; `held` is filled.
    fn draw_image_box_handles(
        &self,
        frame: &mut canvas::Frame,
        position: SheetPoint,
        size: (f64, f64),
        held: Option<ImageHandle>,
        bounds: Rectangle,
    ) {
        let ink = self.handle_ink();
        let paper = crate::render_config::to_iced(&self.prefs.canvas_colors.paper);
        let outline = canvas::Stroke::default()
            .with_color(ink)
            .with_width(HANDLE_LINE_PX);
        let cam = self.camera();
        for handle in ImageHandle::ALL {
            let p = handle.at(position, size);
            let at = cam.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
            let square = canvas::Path::rectangle(
                iced::Point::new(at.x - HANDLE_HALF_PX, at.y - HANDLE_HALF_PX),
                iced::Size::new(2.0 * HANDLE_HALF_PX, 2.0 * HANDLE_HALF_PX),
            );
            frame.fill(&square, if held == Some(handle) { ink } else { paper });
            frame.stroke(&square, outline);
        }
    }

    /// The handles of every selected, unlocked image but the one a drag
    /// holds, which [`Self::draw_image_drag`] draws.
    pub(in crate::canvas) fn draw_image_handles(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        sheet: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        bounds: Rectangle,
    ) {
        let Some(sheet) = sheet else {
            return;
        };
        let dragged = state.image_drag.map(|drag| drag.image);
        for image in selected_images(sheet, &self.selected) {
            if Some(image.uuid) == dragged {
                continue;
            }
            let size = (image.width, image.height);
            self.draw_image_box_handles(frame, image.position, size, None, bounds);
        }
    }

    /// The box a held handle gives, its handles and the size pill.
    pub(in crate::canvas) fn draw_image_drag(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
    ) {
        let Some(drag) = state.image_drag else {
            return;
        };
        let ink = self.handle_ink();
        let (position, size) = drag.now;
        let (a, b) = self.image_box_on_screen(position, size, bounds);
        let rect = canvas::Path::rectangle(
            iced::Point::new(a.x.min(b.x), a.y.min(b.y)),
            iced::Size::new((b.x - a.x).abs(), (b.y - a.y).abs()),
        );
        frame.stroke(
            &rect,
            canvas::Stroke::default()
                .with_color(ink)
                .with_width(HANDLE_LINE_PX),
        );
        self.draw_image_box_handles(frame, position, size, Some(drag.handle), bounds);
        self.draw_image_size_pill(frame, &drag, iced::Point::new(a.x.max(b.x), a.y.min(b.y)));
    }

    /// "96.52 × 46.97 mm · 30.4 %" by the box's top-right corner.
    fn draw_image_size_pill(
        &self,
        frame: &mut canvas::Frame,
        drag: &ImageDrag,
        corner: iced::Point,
    ) {
        let unit = self.prefs.unit;
        let (w, h) = drag.now.1;
        let mut label = format!(
            "{} × {} {unit}",
            signex_types::measure::length_field_text(w, unit),
            signex_types::measure::length_field_text(h, unit),
        );
        let natural = self.active_snapshot().and_then(|sheet| {
            sheet.drawings.iter().find_map(|d| match d {
                signex_types::schematic::SchDrawing::Image(i) if i.uuid == drag.image => {
                    i.natural_size_mm().map(|(nw, _)| nw)
                }
                _ => None,
            })
        });
        if let Some(scale) = drag.scale_percent(natural) {
            label.push_str(&format!(" · {scale:.1} %"));
        }
        let tag = super::hud_tag::layout(corner, &label);
        frame.fill(
            &canvas::Path::rectangle(tag.chip.position(), tag.chip.size()),
            Color::from_rgba(0.0, 0.0, 0.0, 0.75),
        );
        frame.fill_text(canvas::Text {
            content: label,
            position: tag.text_origin,
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.95),
            size: iced::Pixels(super::hud_tag::FONT_SIZE),
            font: crate::render_config::IOSEVKA,
            ..canvas::Text::default()
        });
    }
}
