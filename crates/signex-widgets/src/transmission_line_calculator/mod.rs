mod analysis;
mod chart_geometry;
mod circuit_tokens;
mod element_analysis;
mod model;
#[allow(dead_code)]
mod rust_rf_adapter;
mod s_parameter_analysis;
mod smith_chart_grid;
mod svg;
pub mod tool;
mod touchstone;
mod two_port_solver;

#[cfg(test)]
pub(crate) use analysis::select_active_frequency;
pub(crate) use analysis::solve_frequency_points;
#[cfg(test)]
pub(crate) use analysis::{IMPEDANCE_ARC_SEGMENTS as WEBSITE_IMPEDANCE_ARC_SEGMENTS, TAU};
pub use analysis::{
    analyze_smith_chart, analyze_smith_chart_with_runtime_adjustments, apply_runtime_adjustments,
    solve,
};
pub(crate) use chart_geometry::SPEED_OF_LIGHT_M_PER_S;
pub use chart_geometry::{
    chart_point_from_normalized_impedance, impedance_to_reflection, length_to_meters,
    normalized_impedance_from_chart_point, reflection_to_impedance,
};
#[cfg(test)]
pub(crate) use circuit_tokens::format_number;
pub(crate) use circuit_tokens::{same_number, serialize_circuit_tokens, split_circuit_tokens};
pub use model::*;
#[cfg(test)]
pub(crate) use s_parameter_analysis::{noise_frequency_samples, solve_noise_figure};
pub use s_parameter_analysis::{solve_noise_figure_circles, solve_s_parameter_gain_circles};
pub use svg::render_smith_chart_svg;
pub use touchstone::{
    TouchstoneFormat, parse_touchstone, read_touchstone, serialize_touchstone, write_touchstone,
};
pub use two_port_solver::solve_two_port_s_parameters;

pub const DEFAULT_REFERENCE_IMPEDANCE_OHM: f64 = 50.0;

#[cfg(test)]
#[path = "../../tests/transmission_line_calculator/lib_tests.rs"]
mod lib_tests;
#[cfg(test)]
#[path = "../../tests/transmission_line_calculator/rust_rf_adapter_tests.rs"]
mod rust_rf_adapter_tests;
#[cfg(test)]
#[path = "../../tests/transmission_line_calculator/s_parameter_interpolation_tests.rs"]
mod s_parameter_interpolation_tests;
#[cfg(test)]
#[path = "../../tests/transmission_line_calculator/smith_chart_grid_tests.rs"]
mod smith_chart_grid_tests;
#[cfg(test)]
#[path = "../../tests/transmission_line_calculator/two_port_tests.rs"]
mod two_port_tests;
