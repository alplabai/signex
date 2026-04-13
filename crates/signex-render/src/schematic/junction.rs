//! Junction and no-connect marker rendering.

use iced::Color;
use iced::widget::canvas::{self, path};

use signex_types::schematic::{Junction, NoConnect};

use super::ScreenTransform;

/// KiCad default junction diameter when not specified (1.0mm → 0.5mm radius).
const JUNCTION_DEFAULT_DIAMETER_MM: f64 = 1.0;

/// Draw a junction as a filled circle at its position.
pub fn draw_junction(
    frame: &mut canvas::Frame,
    junction: &Junction,
    transform: &ScreenTransform,
    color: Color,
) {
    let center = transform.to_screen_point(junction.position.x, junction.position.y);
    // Use diameter from file if non-zero, else KiCad default 1.0mm
    let diameter = if junction.diameter > 0.0 {
        junction.diameter
    } else {
        JUNCTION_DEFAULT_DIAMETER_MM
    };
    let radius = transform.world_len(diameter / 2.0).max(2.0);

    let circle = canvas::Path::circle(center, radius);
    frame.fill(&circle, color);
}

/// Half-size of the no-connect X in world mm.
const NO_CONNECT_SIZE_MM: f64 = 1.0;

/// Draw a no-connect marker as an X at its position.
pub fn draw_no_connect(
    frame: &mut canvas::Frame,
    nc: &NoConnect,
    transform: &ScreenTransform,
    color: Color,
) {
    let cx = nc.position.x;
    let cy = nc.position.y;
    let d = NO_CONNECT_SIZE_MM;

    let p1a = transform.to_screen_point(cx - d, cy - d);
    let p1b = transform.to_screen_point(cx + d, cy + d);
    let p2a = transform.to_screen_point(cx - d, cy + d);
    let p2b = transform.to_screen_point(cx + d, cy - d);

    let path = canvas::Path::new(|b: &mut path::Builder| {
        b.move_to(p1a);
        b.line_to(p1b);
        b.move_to(p2a);
        b.line_to(p2b);
    });

    let width = (transform.scale * 0.25).clamp(1.0, 3.0);
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width);
    frame.stroke(&path, stroke);
}
