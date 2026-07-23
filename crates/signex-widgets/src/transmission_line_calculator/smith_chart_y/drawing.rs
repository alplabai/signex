use iced::widget::canvas;
use iced::{Color, Point};

use crate::transmission_line_calculator::Complex;
use crate::transmission_line_calculator::tool::smith_chart_2d::{
    draw_label, draw_polyline, draw_smith_chart_grid,
};

use super::admittance_chart_point;

/// Draws admittance grid into the target drawing surface.
pub(in crate::transmission_line_calculator::tool) fn draw_admittance_grid(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    conductance_labels: &[f64],
    susceptance_labels: &[f64],
    show_labels: bool,
) {
    draw_smith_chart_grid(
        frame,
        center,
        radius,
        conductance_labels,
        susceptance_labels,
        true,
    );
    if show_labels {
        draw_admittance_grid_labels(
            frame,
            center,
            radius,
            conductance_labels,
            susceptance_labels,
        );
    }
}

/// Draws admittance Q arc into the target drawing surface.
pub(in crate::transmission_line_calculator::tool) fn draw_admittance_q_arc(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    q: f64,
    color: Color,
) {
    let points = (0..=240)
        .map(|index| {
            let conductance = 10.0_f64.powf(-2.0 + index as f64 / 60.0);
            admittance_chart_point(Complex::new(conductance, conductance * q))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, color, 0.8);
}

/// Draws admittance grid labels into the target drawing surface.
fn draw_admittance_grid_labels(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    conductance_labels: &[f64],
    susceptance_labels: &[f64],
) {
    for conductance in conductance_labels
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
    {
        let (x, y) = admittance_chart_point(Complex::new(conductance, 0.0));
        draw_label(
            frame,
            format!("{conductance:.1}"),
            Point::new(
                center.x + x as f32 * radius - 8.0,
                center.y - y as f32 * radius - 14.0,
            ),
            Color::from_rgba8(181, 218, 199, 0.82),
        );
    }
    for susceptance in susceptance_labels.iter().copied() {
        let (x, y) = admittance_chart_point(Complex::new(1.0, susceptance));
        draw_label(
            frame,
            format!("{susceptance:+.1}j"),
            Point::new(
                center.x + x as f32 * radius + 4.0,
                center.y - y as f32 * radius - 4.0,
            ),
            Color::from_rgba8(178, 220, 214, 0.82),
        );
    }
}
