//! The Measure tool's overlay (#1256): a line between the two points with a
//! dx / dy / distance chip, live while the second point is pending and kept
//! once it is placed.

use super::super::*;
use signex_types::measure::Measurement;
use signex_types::schematic::Point;

impl SchematicCanvas<'_> {
    /// The segment `world_a`..`world_b` with its readout chip at the middle.
    pub(in crate::canvas) fn draw_measure_segment(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
        world_a: Point,
        world_b: Point,
    ) {
        let cam = self.camera();
        let to_screen =
            |p: Point| cam.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
        let (a, b) = (to_screen(world_a), to_screen(world_b));
        let accent = Color::from_rgb(0.94, 0.74, 0.28);
        frame.stroke(
            &canvas::Path::line(a, b),
            canvas::Stroke::default().with_color(accent).with_width(1.5),
        );
        frame.fill(&canvas::Path::circle(a, 3.0), accent);
        frame.fill(&canvas::Path::circle(b, 3.0), accent);

        let label = Measurement::between(world_a, world_b).describe(self.prefs.unit);
        let middle = iced::Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let tag = super::hud_tag::layout(middle, &label);
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

    /// The last finished measurement, drawn whether or not the cursor is over
    /// the canvas.
    pub(in crate::canvas) fn draw_measure_result(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
    ) {
        if let Some(result) = &self.measure {
            self.draw_measure_segment(frame, bounds, result.a, result.b);
        }
    }
}
