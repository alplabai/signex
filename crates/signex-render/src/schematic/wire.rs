//! Wire, bus, and bus-entry rendering.

use iced::widget::canvas::{self, path};
use iced::Color;

use signex_types::schematic::{Bus, BusEntry, Wire};

use super::ScreenTransform;

/// Draw a wire as a line from start to end.
pub fn draw_wire(
    frame: &mut canvas::Frame,
    wire: &Wire,
    transform: &ScreenTransform,
    color: Color,
) {
    let p1 = transform.to_screen_point(wire.start.x, wire.start.y);
    let p2 = transform.to_screen_point(wire.end.x, wire.end.y);

    let line = canvas::Path::line(p1, p2);
    let width = (transform.scale * 0.25).max(1.0).min(4.0);
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width);
    frame.stroke(&line, stroke);
}

/// Draw a bus as a thicker line from start to end.
pub fn draw_bus(
    frame: &mut canvas::Frame,
    bus: &Bus,
    transform: &ScreenTransform,
    color: Color,
) {
    let p1 = transform.to_screen_point(bus.start.x, bus.start.y);
    let p2 = transform.to_screen_point(bus.end.x, bus.end.y);

    let line = canvas::Path::line(p1, p2);
    // Buses are drawn thicker than wires
    let width = (transform.scale * 0.5).max(2.0).min(6.0);
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width);
    frame.stroke(&line, stroke);
}

/// Draw a bus entry as a short diagonal line.
pub fn draw_bus_entry(
    frame: &mut canvas::Frame,
    entry: &BusEntry,
    transform: &ScreenTransform,
    color: Color,
) {
    let p1 = transform.to_screen_point(entry.position.x, entry.position.y);
    let p2 = transform.to_screen_point(
        entry.position.x + entry.size.0,
        entry.position.y + entry.size.1,
    );

    let path = canvas::Path::new(|b: &mut path::Builder| {
        b.move_to(p1);
        b.line_to(p2);
    });

    let width = (transform.scale * 0.25).max(1.0).min(4.0);
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width);
    frame.stroke(&path, stroke);
}
