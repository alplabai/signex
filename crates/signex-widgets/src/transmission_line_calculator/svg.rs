use crate::transmission_line_calculator::smith_chart_grid::{
    SmithChartGridHierarchy, SmithChartGridLineKind, smith_chart_grid,
};
use crate::transmission_line_calculator::*;
use std::fmt::Write as _;

/// Renders smith chart SVG into the requested output format.
pub fn render_smith_chart_svg(
    result: Option<&SolveResult>,
    options: SmithChartSvgOptions,
) -> String {
    let width = options.width.max(64.0);
    let height = options.height.max(64.0);
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let radius = width.min(height) * 0.44;
    let mut svg = String::new();
    write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.3} {height:.3}" width="{width:.0}" height="{height:.0}" role="img" aria-label="Smith chart">"#
    )
    .expect("write to string cannot fail");
    write!(
        svg,
        r##"<rect x="0" y="0" width="{width:.3}" height="{height:.3}" fill="#14181e"/>"##
    )
    .expect("write to string cannot fail");
    write!(
        svg,
        r#"<defs><clipPath id="smith-chart-disk"><circle cx="{center_x:.3}" cy="{center_y:.3}" r="{radius:.3}"/></clipPath></defs>"#
    )
    .expect("write to string cannot fail");
    push_svg_circle(
        &mut svg, center_x, center_y, radius, "none", "#b2becc", 1.5, 1.0,
    );
    push_svg_line(
        &mut svg,
        center_x - radius,
        center_y,
        center_x + radius,
        center_y,
        "#525c69",
        1.0,
    );
    push_svg_line(
        &mut svg,
        center_x,
        center_y - radius,
        center_x,
        center_y + radius,
        "#3d4550",
        0.8,
    );

    if options.show_grid {
        push_svg_smith_chart_grid(
            &mut svg,
            center_x,
            center_y,
            radius,
            false,
            "smith-chart-grid",
        );
        push_svg_grid_labels(
            &mut svg,
            center_x,
            center_y,
            radius,
            &options.resistance_labels,
            &options.reactance_labels,
        );
    }
    if options.show_admittance {
        push_svg_smith_chart_grid(
            &mut svg,
            center_x,
            center_y,
            radius,
            true,
            "admittance-smith-chart-grid",
        );
    }
    if options.show_vswr {
        for vswr in options
            .vswr_circles
            .iter()
            .copied()
            .filter(|value| *value > 1.0)
        {
            let magnitude = (vswr - 1.0) / (vswr + 1.0);
            push_svg_circle(
                &mut svg,
                center_x,
                center_y,
                radius * magnitude,
                "none",
                "#d7a44a",
                0.8,
                0.45,
            );
            push_svg_text(
                &mut svg,
                center_x + radius * magnitude + 4.0,
                center_y - 3.0,
                &format!("VSWR {vswr:.1}"),
                "#e5b863",
                0.72,
            );
        }
    }
    if options.show_q {
        for q in options
            .q_circles
            .iter()
            .copied()
            .filter(|value| *value > 0.0)
        {
            let points = q_points(q);
            push_polyline(
                &mut svg, center_x, center_y, radius, &points, "#5fbeaa", 0.8, 0.42,
            );
            let points = q_points(-q);
            push_polyline(
                &mut svg, center_x, center_y, radius, &points, "#5fbeaa", 0.8, 0.42,
            );
            let (x, y) = chart_point_from_normalized_impedance(Complex::new(1.0, q));
            push_svg_text(
                &mut svg,
                center_x + x * radius + 4.0,
                center_y - y * radius,
                &format!("Q {q:.1}"),
                "#7cd8c8",
                0.7,
            );
        }
    }
    for circle in &options.stability_circles {
        push_svg_chart_circle(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.source_center,
            circle.source_radius,
            "#ff8a5c",
            0.65,
        );
        push_svg_chart_circle(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.load_center,
            circle.load_radius,
            "#7aa7ff",
            0.65,
        );
    }
    for circle in &options.gain_circles {
        let stroke = match circle.port {
            GainCirclePort::Input => "#c79cff",
            GainCirclePort::Output => "#6fcbff",
        };
        push_svg_chart_circle(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.center,
            circle.radius,
            stroke,
            0.62,
        );
        let label = match circle.port {
            GainCirclePort::Input => format!("{:.1} dB in", circle.target_gain_db),
            GainCirclePort::Output => format!("{:.1} dB out", circle.target_gain_db),
        };
        push_svg_chart_circle_label(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.center,
            circle.radius,
            &label,
            stroke,
        );
    }
    for circle in &options.noise_figure_circles {
        push_svg_chart_circle(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.center,
            circle.radius,
            "#7cd8c8",
            0.62,
        );
        push_svg_chart_circle_label(
            &mut svg,
            center_x,
            center_y,
            radius,
            circle.center,
            circle.radius,
            &format!("{:.1} dB NF", circle.target_noise_figure_db),
            "#7cd8c8",
        );
    }
    for marker in &options.z_markers {
        let (x, y) = chart_point_from_normalized_impedance(
            *marker * (1.0 / options.reference_impedance_ohm),
        );
        let marker_x = center_x + x * radius;
        let marker_y = center_y - y * radius;
        push_svg_circle(
            &mut svg, marker_x, marker_y, 4.0, "#f4da76", "#f4da76", 1.0, 1.0,
        );
        push_svg_text(
            &mut svg,
            marker_x + 6.0,
            marker_y - 6.0,
            &format!("{:.1}{:+.1}j", marker.re, marker.im),
            "#f4da76",
            1.0,
        );
    }
    for trace in &options.impedance_arc_traces {
        let points = trace
            .points
            .iter()
            .filter(|point| point.magnitude() <= 1.001)
            .map(|point| (point.re, point.im))
            .collect::<Vec<_>>();
        push_polyline(
            &mut svg,
            center_x,
            center_y,
            radius,
            &points,
            &trace.color,
            1.3,
            0.8,
        );
        if let Some((x, y)) = points.last() {
            push_svg_text(
                &mut svg,
                center_x + x * radius + 5.0,
                center_y - y * radius - 5.0,
                &trace.label,
                &trace.color,
                0.82,
            );
        }
    }
    for trace in &options.s_parameter_traces {
        let points = trace
            .points
            .iter()
            .filter(|point| point.magnitude() <= 1.001)
            .map(|point| (point.re, point.im))
            .collect::<Vec<_>>();
        push_polyline(
            &mut svg,
            center_x,
            center_y,
            radius,
            &points,
            &trace.color,
            1.2,
            0.9,
        );
        if let Some((x, y)) = points.last() {
            push_svg_text(
                &mut svg,
                center_x + x * radius + 5.0,
                center_y - y * radius - 5.0,
                &trace.label,
                &trace.color,
                0.95,
            );
        }
    }
    if let Some(result) = result {
        push_svg_circle(
            &mut svg,
            center_x + result.reflection_coefficient.re * radius,
            center_y - result.reflection_coefficient.im * radius,
            5.0,
            "#ee5c52",
            "#ffffff",
            1.0,
            1.0,
        );
    }
    svg.push_str("</svg>");
    svg
}

/// Appends the conventional Smith-chart grid to SVG output.
fn push_svg_smith_chart_grid(
    svg: &mut String,
    center_x: f64,
    center_y: f64,
    radius: f64,
    reflected: bool,
    id: &str,
) {
    write!(svg, r#"<g id="{id}" clip-path="url(#smith-chart-disk)">"#)
        .expect("write to string cannot fail");
    for line in smith_chart_grid() {
        let (hierarchy, color, width, opacity) = match (reflected, line.hierarchy) {
            (false, SmithChartGridHierarchy::Major) => ("major", "#4a6f9e", 0.85, 0.72),
            (false, SmithChartGridHierarchy::Minor) => ("minor", "#3a567a", 0.45, 0.28),
            (true, SmithChartGridHierarchy::Major) => ("major", "#4a8970", 0.8, 0.64),
            (true, SmithChartGridHierarchy::Minor) => ("minor", "#456f5b", 0.4, 0.24),
        };
        let (kind, value) = match line.kind {
            SmithChartGridLineKind::Resistance { value } => ("resistance", value),
            SmithChartGridLineKind::Reactance { value } => ("reactance", value),
        };
        write!(
            svg,
            r#"<polyline data-grid-kind="{kind}" data-grid-value="{value:.6}" data-grid-hierarchy="{hierarchy}" fill="none" stroke="{color}" stroke-width="{width:.3}" opacity="{opacity:.3}" points=""#
        )
        .expect("write to string cannot fail");
        for (x, y) in &line.points {
            let direction = if reflected { -1.0 } else { 1.0 };
            write!(
                svg,
                "{:.3},{:.3} ",
                center_x + direction * x * radius,
                center_y - direction * y * radius,
            )
            .expect("write to string cannot fail");
        }
        svg.push_str(r#""/>"#);
    }
    svg.push_str("</g>");
}

/// Computes the Q sample points.
fn q_points(q: f64) -> Vec<(f64, f64)> {
    (0..=240)
        .map(|idx| {
            let resistance = 10.0_f64.powf(-2.0 + f64::from(idx) / 60.0);
            chart_point_from_normalized_impedance(Complex::new(resistance, resistance * q))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect()
}

/// Appends polyline to the destination collection or output.
fn push_polyline(
    svg: &mut String,
    center_x: f64,
    center_y: f64,
    radius: f64,
    points: &[(f64, f64)],
    color: &str,
    width: f64,
    opacity: f64,
) {
    if points.len() < 2 {
        return;
    }
    svg.push_str(r#"<polyline fill="none" points=""#);
    for (x, y) in points {
        write!(
            svg,
            "{:.3},{:.3} ",
            center_x + x * radius,
            center_y - y * radius
        )
        .expect("write to string cannot fail");
    }
    write!(
        svg,
        r#"" stroke="{color}" stroke-width="{width:.3}" opacity="{opacity:.3}"/>"#
    )
    .expect("write to string cannot fail");
}

/// Appends SVG grid labels to the destination collection or output.
fn push_svg_grid_labels(
    svg: &mut String,
    center_x: f64,
    center_y: f64,
    radius: f64,
    resistance_labels: &[f64],
    reactance_labels: &[f64],
) {
    for resistance in resistance_labels
        .iter()
        .copied()
        .filter(|resistance| resistance.is_finite() && *resistance >= 0.0)
    {
        let (x, y) = chart_point_from_normalized_impedance(Complex::new(resistance, 0.0));
        push_svg_text(
            svg,
            center_x + x * radius - 8.0,
            center_y - y * radius - 14.0,
            &format!("{resistance:.1}"),
            "#b6cde8",
            0.78,
        );
    }
    for reactance in reactance_labels
        .iter()
        .copied()
        .filter(|reactance| reactance.is_finite() && *reactance != 0.0)
    {
        let (x, y) = chart_point_from_normalized_impedance(Complex::new(1.0, reactance));
        push_svg_text(
            svg,
            center_x + x * radius + 4.0,
            center_y - y * radius - 4.0,
            &format!("{reactance:+.1}j"),
            "#c7b8e8",
            0.78,
        );
    }
}

/// Appends SVG text to the destination collection or output.
fn push_svg_text(svg: &mut String, x: f64, y: f64, content: &str, fill: &str, opacity: f64) {
    write!(
        svg,
        r#"<text x="{x:.3}" y="{y:.3}" fill="{fill}" opacity="{opacity:.3}" font-size="10" font-family="sans-serif">{content}</text>"#
    )
    .expect("write to string cannot fail");
}

/// Appends SVG chart circle to the destination collection or output.
fn push_svg_chart_circle(
    svg: &mut String,
    center_x: f64,
    center_y: f64,
    radius: f64,
    chart_center: Complex,
    chart_radius: f64,
    stroke: &str,
    opacity: f64,
) {
    if !chart_radius.is_finite() || chart_radius <= 0.0 {
        return;
    }
    push_svg_circle(
        svg,
        center_x + chart_center.re * radius,
        center_y - chart_center.im * radius,
        chart_radius * radius,
        "none",
        stroke,
        1.2,
        opacity,
    );
}

/// Appends SVG chart circle label to the destination collection or output.
fn push_svg_chart_circle_label(
    svg: &mut String,
    center_x: f64,
    center_y: f64,
    radius: f64,
    chart_center: Complex,
    chart_radius: f64,
    label: &str,
    fill: &str,
) {
    if !chart_radius.is_finite() || chart_radius <= 0.0 {
        return;
    }
    push_svg_text(
        svg,
        center_x + chart_center.re * radius,
        center_y - chart_center.im * radius - chart_radius * radius - 4.0,
        label,
        fill,
        0.9,
    );
}

/// Appends SVG circle to the destination collection or output.
fn push_svg_circle(
    svg: &mut String,
    cx: f64,
    cy: f64,
    r: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
    opacity: f64,
) {
    write!(
        svg,
        r#"<circle cx="{cx:.3}" cy="{cy:.3}" r="{r:.3}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width:.3}" opacity="{opacity:.3}"/>"#
    )
    .expect("write to string cannot fail");
}

/// Appends SVG line to the destination collection or output.
fn push_svg_line(
    svg: &mut String,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stroke: &str,
    stroke_width: f64,
) {
    write!(
        svg,
        r#"<line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="{stroke}" stroke-width="{stroke_width:.3}"/>"#
    )
    .expect("write to string cannot fail");
}
