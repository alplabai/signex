//! Canvas path and fill for a [`GpuPolygon`], shared by the CPU replay paths.

use iced::Point;
use iced::widget::canvas;
use signex_gfx::primitive::polygon::GpuPolygon;

/// The closed outline of `polygon` plus one extra closed subpath per hole of
/// 3 or more vertices, and the fill that leaves those holes empty. The rule is
/// even-odd only when a hole is present, so a hole-less polygon keeps the
/// default non-zero fill. Callers draw the same path for the stroke, which
/// therefore outlines the holes too. Needs `polygon.vertices.len() >= 3`.
pub fn polygon_path_and_fill(
    polygon: &GpuPolygon,
    to_screen: impl Fn([f32; 2]) -> Point,
    color: iced::Color,
) -> (canvas::Path, canvas::Fill) {
    let holes: Vec<&Vec<[f32; 2]>> = polygon
        .holes
        .iter()
        .filter(|hole| hole.len() >= 3)
        .collect();
    let path = canvas::Path::new(|builder| {
        for ring in std::iter::once(&polygon.vertices).chain(holes.iter().copied()) {
            let mut points = ring.iter().map(|vertex| to_screen(*vertex));
            if let Some(first) = points.next() {
                builder.move_to(first);
                points.for_each(|point| builder.line_to(point));
                builder.close();
            }
        }
    });
    let mut fill = canvas::Fill::from(color);
    if !holes.is_empty() {
        fill.rule = canvas::fill::Rule::EvenOdd;
    }
    (path, fill)
}
