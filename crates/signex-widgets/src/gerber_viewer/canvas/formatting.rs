use super::*;

pub(in crate::gerber_viewer) fn format_cartesian_coordinate_in_unit(
    point: signex_gerber::Point,
    unit: GerberDisplayUnit,
    decimal_separator: &str,
) -> String {
    format!(
        "X: {}  Y: {} {}",
        unit.format_value(point.x, decimal_separator),
        unit.format_value(point.y, decimal_separator),
        unit.suffix(),
    )
}

pub(in crate::gerber_viewer) fn format_polar_coordinate_in_unit(
    point: signex_gerber::Point,
    unit: GerberDisplayUnit,
    decimal_separator: &str,
) -> String {
    let radius = point.x.hypot(point.y);
    let angle_radians = point.y.atan2(point.x);
    let angle_degrees = angle_radians.to_degrees();
    format!(
        "R: {} {}  θ: {}° / {} rad",
        unit.format_value(radius, decimal_separator),
        unit.suffix(),
        format!("{angle_degrees:.2}").replace('.', decimal_separator),
        format!("{angle_radians:.4}").replace('.', decimal_separator),
    )
}

pub(in crate::gerber_viewer) fn format_bounds_in_unit(
    bounds: Bounds,
    unit: GerberDisplayUnit,
    decimal_separator: &str,
) -> String {
    format!(
        "Bounds: X {}…{}  Y {}…{} {}",
        unit.format_value(bounds.min.x, decimal_separator),
        unit.format_value(bounds.max.x, decimal_separator),
        unit.format_value(bounds.min.y, decimal_separator),
        unit.format_value(bounds.max.y, decimal_separator),
        unit.suffix(),
    )
}
