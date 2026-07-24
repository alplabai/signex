use crate::transmission_line_calculator::{Complex, chart_point_from_normalized_impedance};

mod drawing;
mod smith_chart_y_canvas;

pub(in crate::transmission_line_calculator::tool) use drawing::{
    draw_admittance_grid, draw_admittance_q_arc,
};
pub(super) use smith_chart_y_canvas::SmithChartYCanvas;

/// Maps normalized admittance to the mirrored Smith-chart coordinates.
pub(in crate::transmission_line_calculator::tool) fn admittance_chart_point(
    normalized_admittance: Complex,
) -> (f64, f64) {
    let (x, y) = chart_point_from_normalized_impedance(normalized_admittance);
    (-x, -y)
}

#[cfg(test)]
#[path = "../../../tests/transmission_line_calculator/smith_chart_y_tests.rs"]
mod tests;
