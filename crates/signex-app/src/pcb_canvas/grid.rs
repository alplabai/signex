//! The board grid behind the PCB canvas. Its lines sit on board multiples
//! of the pitch, so on a turned view (#2316) they turn with the board.

use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};

use super::camera::BoardCamera;

/// Stroke the grid lines `step_px` apart over a `size` frame.
pub(super) fn draw_grid(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    step_px: f32,
    color: Color,
) {
    let stroke = Stroke::default().with_color(color).with_width(0.5);
    if camera.rotation().is_identity() {
        let offset = camera.offset();
        let mut x = offset.x.rem_euclid(step_px) - step_px;
        while x <= size.width + step_px {
            frame.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, size.height)),
                stroke,
            );
            x += step_px;
        }
        let mut y = offset.y.rem_euclid(step_px) - step_px;
        while y <= size.height + step_px {
            frame.stroke(
                &Path::line(Point::new(0.0, y), Point::new(size.width, y)),
                stroke,
            );
            y += step_px;
        }
        return;
    }
    let Some((min, max)) = camera.visible_world_bounds(size) else {
        return;
    };
    let step = step_px / camera.scale();
    let line =
        |a: Point, b: Point| Path::line(camera.world_to_screen(a), camera.world_to_screen(b));
    let mut k = (min[0] / step).floor();
    while k * step <= max[0] {
        let x = k * step;
        frame.stroke(&line(Point::new(x, min[1]), Point::new(x, max[1])), stroke);
        k += 1.0;
    }
    let mut k = (min[1] / step).floor();
    while k * step <= max[1] {
        let y = k * step;
        frame.stroke(&line(Point::new(min[0], y), Point::new(max[0], y)), stroke);
        k += 1.0;
    }
}
