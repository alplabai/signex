use iced::widget::canvas;
use iced::{Color, Point};

use crate::Complex;
use crate::tool::smith_chart_2d::{draw_label, draw_polyline};

use super::admittance_chart_point;

pub(in crate::tool) fn draw_admittance_grid(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    conductance_labels: &[f64],
    susceptance_labels: &[f64],
    show_labels: bool,
) {
    for conductance in conductance_labels
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
    {
        draw_conductance_circle(frame, center, radius, conductance);
    }
    for susceptance in susceptance_labels.iter().copied() {
        draw_susceptance_arc(frame, center, radius, susceptance);
    }
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

pub(in crate::tool) fn draw_admittance_q_arc(
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

fn draw_conductance_circle(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    conductance: f64,
) {
    let chart_center = conductance / (conductance + 1.0);
    let chart_radius = 1.0 / (conductance + 1.0);
    let circle = canvas::Path::circle(
        Point::new(center.x - chart_center as f32 * radius, center.y),
        chart_radius as f32 * radius,
    );
    frame.stroke(
        &circle,
        canvas::Stroke::default()
            .with_width(0.7)
            .with_color(Color::from_rgb8(69, 111, 91)),
    );
}

fn draw_susceptance_arc(frame: &mut canvas::Frame, center: Point, radius: f32, susceptance: f64) {
    let points = (-200..=200)
        .map(|index| {
            let conductance = 10.0_f64.powf(index as f64 / 80.0);
            admittance_chart_point(Complex::new(conductance, susceptance))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(
        frame,
        center,
        radius,
        &points,
        Color::from_rgb8(75, 119, 111),
        0.7,
    );
}

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
