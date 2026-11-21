//! The watermark drawn over the preview page (#1298). The rasterised page
//! carries no watermark; this canvas draws it at `Watermark::placement`,
//! the rule the PDF writer uses too, scaled to the page image, so the
//! preview always shows what will print.

use iced::widget::canvas::{self, Frame, Geometry, Program, Text};
use iced::{Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use signex_output::Watermark;

/// Everything the overlay needs, read from the state by the view.
#[derive(Debug, Clone)]
pub(super) struct WatermarkOverlay {
    pub watermark: Watermark,
    /// The line under the text, when it prints.
    pub stamp: Option<String>,
    /// The page in mm, as printed.
    pub page_mm: (f64, f64),
}

/// The rectangle a page of `page_mm` fills when fitted, centred, into
/// `bounds`: the image widget's Contain fit.
pub(super) fn contain_rect(bounds: Size, page_mm: (f64, f64)) -> Rectangle {
    let (w_mm, h_mm) = (page_mm.0 as f32, page_mm.1 as f32);
    if w_mm <= 0.0 || h_mm <= 0.0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Rectangle::new(Point::ORIGIN, Size::ZERO);
    }
    let scale = (bounds.width / w_mm).min(bounds.height / h_mm);
    let size = Size::new(w_mm * scale, h_mm * scale);
    Rectangle::new(
        Point::new(
            (bounds.width - size.width) / 2.0,
            (bounds.height - size.height) / 2.0,
        ),
        size,
    )
}

impl<M> Program<M> for WatermarkOverlay {
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
        let page = contain_rect(bounds.size(), self.page_mm);
        if page.width <= 0.0 {
            return vec![frame.into_geometry()];
        }
        let px_per_mm = page.width / self.page_mm.0 as f32;
        let place = self.watermark.placement(self.page_mm.0, self.page_mm.1);
        let [r, g, b] = self.watermark.colour_rgb;
        let colour = Color {
            a: f32::from(self.watermark.opacity_percent) / 100.0,
            ..Color::from_rgb8(r, g, b)
        };
        let bold = Font {
            weight: iced::font::Weight::Bold,
            ..Font::DEFAULT
        };
        let text_px = place.text_height_mm as f32 * px_per_mm;
        let centre = Vector::new(
            page.x + place.center_mm.0 as f32 * px_per_mm,
            page.y + place.center_mm.1 as f32 * px_per_mm,
        );
        frame.with_save(|frame| {
            frame.translate(centre);
            // Counter-clockwise as seen on the page; the y axis points down.
            frame.rotate(-(place.angle_deg as f32).to_radians());
            frame.fill_text(Text {
                content: self.watermark.text.printed().to_owned(),
                position: Point::ORIGIN,
                color: colour,
                size: Pixels(text_px),
                font: bold,
                align_x: iced::Alignment::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Text::default()
            });
            if let Some(stamp) = &self.stamp {
                let stamp_px = place.revision_height_mm as f32 * px_per_mm;
                let below =
                    text_px / 2.0 + place.revision_gap_mm as f32 * px_per_mm + stamp_px / 2.0;
                frame.fill_text(Text {
                    content: stamp.clone(),
                    position: Point::new(0.0, below),
                    color: colour,
                    size: Pixels(stamp_px),
                    font: bold,
                    align_x: iced::Alignment::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    ..Text::default()
                });
            }
        });
        vec![frame.into_geometry()]
    }
}

/// The overlay as a canvas filling its slot over the page image.
pub(super) fn view<'a, M: 'a>(overlay: WatermarkOverlay) -> iced::Element<'a, M> {
    use iced::Widget as _;
    canvas::Canvas::new(overlay)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_landscape_page_fits_by_its_width_in_a_tall_slot() {
        let rect = contain_rect(Size::new(297.0, 400.0), (297.0, 210.0));
        assert_eq!(rect.width, 297.0);
        assert_eq!(rect.height, 210.0);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 95.0);
    }

    #[test]
    fn a_page_fits_by_its_height_in_a_wide_slot() {
        let rect = contain_rect(Size::new(1000.0, 105.0), (297.0, 210.0));
        assert_eq!(rect.height, 105.0);
        assert!((rect.width - 148.5).abs() < 1e-3);
        assert!((rect.x - (1000.0 - 148.5) / 2.0).abs() < 1e-3);
    }

    #[test]
    fn an_empty_slot_or_page_draws_nothing() {
        assert_eq!(contain_rect(Size::ZERO, (297.0, 210.0)).width, 0.0);
        assert_eq!(contain_rect(Size::new(10.0, 10.0), (0.0, 210.0)).width, 0.0);
    }
}
