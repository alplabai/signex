use crate::{Complex, chart_point_from_normalized_impedance};

mod drawing;
mod smith_chart_y_canvas;

pub(in crate::tool) use drawing::{draw_admittance_grid, draw_admittance_q_arc};
pub(super) use smith_chart_y_canvas::SmithChartYCanvas;

pub(in crate::tool) fn admittance_chart_point(normalized_admittance: Complex) -> (f64, f64) {
    let (x, y) = chart_point_from_normalized_impedance(normalized_admittance);
    (-x, -y)
}

#[cfg(test)]
mod tests;
