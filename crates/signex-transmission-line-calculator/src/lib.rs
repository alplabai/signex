mod analysis;
mod chart_geometry;
mod model;
mod svg;
pub mod tool;
mod touchstone;
mod url_codec;
pub(crate) mod url_numbers;

#[cfg(test)]
pub(crate) use analysis::{
    IMPEDANCE_ARC_SEGMENTS as WEBSITE_IMPEDANCE_ARC_SEGMENTS, TAU, noise_frequency_samples,
    solve_noise_figure,
};
pub use analysis::{
    analyze_smith_chart, analyze_smith_chart_with_runtime_adjustments, apply_runtime_adjustments,
    solve, solve_noise_figure_circles, solve_s_parameter_gain_circles,
};
pub(crate) use analysis::{select_active_frequency, solve_frequency_points};
pub(crate) use chart_geometry::SPEED_OF_LIGHT_M_PER_S;
pub use chart_geometry::{
    chart_point_from_normalized_impedance, impedance_to_reflection, length_to_meters,
    normalized_impedance_from_chart_point, reflection_to_impedance,
};
pub use model::*;
pub use svg::render_smith_chart_svg;
pub use touchstone::parse_touchstone;
#[cfg(test)]
pub(crate) use url_codec::format_number;
pub(crate) use url_codec::same_number;
pub use url_codec::{
    parse_online_smith_chart_circuit_tokens, parse_online_smith_chart_query,
    serialize_online_smith_chart_circuit_tokens, serialize_online_smith_chart_query,
    split_online_smith_chart_circuit_tokens,
};

pub const DEFAULT_REFERENCE_IMPEDANCE_OHM: f64 = 50.0;

#[cfg(test)]
mod lib_tests;
