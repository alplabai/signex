use super::*;
use crate::tool::smith_chart_y::{
    admittance_chart_point, draw_admittance_grid, draw_admittance_q_arc,
};

mod frequency_plot_canvas;
mod impedance_arc_trace;
mod plot_track;
mod s_parameter_trace;
mod smith_chart_canvas;

pub(super) use frequency_plot_canvas::FrequencyPlotCanvas;
pub(super) use impedance_arc_trace::ImpedanceArcTrace;
pub(super) use plot_track::PlotTrack;
pub(super) use s_parameter_trace::SParameterTrace;
pub(super) use smith_chart_canvas::SmithChartCanvas;

#[cfg(test)]
mod tests;

pub(super) fn push_plot_track(
    tracks: &mut Vec<PlotTrack>,
    label: impl Into<String>,
    points: Vec<(f64, f64)>,
    color: Color,
) {
    if points.len() >= 2 {
        tracks.push(PlotTrack {
            label: label.into(),
            points,
            color,
        });
    }
}

pub(super) fn s_parameter_chart_traces(state: &SmithChartState) -> Vec<SParameterTrace> {
    let Ok(circuit) = state.active_circuit() else {
        return Vec::new();
    };
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };

    let mut traces = Vec::new();
    if state.show_s11_trace {
        push_s_parameter_trace(
            &mut traces,
            "S11",
            Color::from_rgb8(0, 114, 178),
            block.points.iter().map(|point| point.s11),
            state.conjugate_s_parameter_traces,
        );
    }
    if state.show_s21_trace {
        push_s_parameter_trace(
            &mut traces,
            "S21",
            Color::from_rgb8(230, 159, 0),
            block.points.iter().filter_map(|point| point.s21),
            state.conjugate_s_parameter_traces,
        );
    }
    if state.show_s12_trace {
        push_s_parameter_trace(
            &mut traces,
            "S12",
            Color::from_rgb8(204, 121, 167),
            block.points.iter().filter_map(|point| point.s12),
            state.conjugate_s_parameter_traces,
        );
    }
    if state.show_s22_trace {
        push_s_parameter_trace(
            &mut traces,
            "S22",
            Color::from_rgb8(0, 158, 115),
            block.points.iter().filter_map(|point| point.s22),
            state.conjugate_s_parameter_traces,
        );
    }
    traces
}

pub(super) fn impedance_arc_chart_traces(
    result: &SmithChartAnalysis,
    reference_impedance_ohm: f64,
) -> Vec<ImpedanceArcTrace> {
    result
        .impedance_arcs
        .iter()
        .filter_map(|arc| {
            let points = arc
                .points
                .iter()
                .map(|point| {
                    let normalized = *point * (1.0 / reference_impedance_ohm);
                    let (x, y) = chart_point_from_normalized_impedance(normalized);
                    Complex::new(x, y)
                })
                .filter(|point| point.re.is_finite() && point.im.is_finite())
                .collect::<Vec<_>>();
            (points.len() >= 2).then(|| ImpedanceArcTrace {
                label: if arc.variant_index == 0 {
                    arc.element_name.clone()
                } else {
                    format!("{} tol {}", arc.element_name, arc.variant_index)
                },
                color: impedance_arc_color(arc.variant_index),
                points,
            })
        })
        .collect()
}

fn impedance_arc_color(variant_index: usize) -> Color {
    if variant_index == 0 {
        Color::from_rgb8(238, 92, 82)
    } else if variant_index % 2 == 0 {
        Color::from_rgb8(229, 184, 99)
    } else {
        Color::from_rgb8(116, 203, 255)
    }
}

fn push_s_parameter_trace(
    traces: &mut Vec<SParameterTrace>,
    label: &'static str,
    color: Color,
    points: impl IntoIterator<Item = Complex>,
    conjugate: bool,
) {
    let points = points
        .into_iter()
        .map(|point| if conjugate { point.conjugate() } else { point })
        .filter(|point| point.re.is_finite() && point.im.is_finite())
        .collect::<Vec<_>>();
    if points.len() >= 2 {
        traces.push(SParameterTrace {
            label,
            color,
            points,
        });
    }
}

pub(super) fn color_to_svg_hex(color: Color) -> String {
    let channel = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

impl canvas::Program<SmithChartMessage> for FrequencyPlotCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = canvas::Cache::new().draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::from_rgb8(20, 24, 30));

            if self.tracks.is_empty() {
                return;
            }

            let track_height = bounds.height / self.tracks.len() as f32;
            for (index, track) in self.tracks.iter().enumerate() {
                let top = index as f32 * track_height + 8.0;
                let height = (track_height - 34.0).max(24.0);
                draw_frequency_track(
                    frame,
                    bounds.width,
                    top,
                    height,
                    &track.label,
                    &track.points,
                    track.color,
                );
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    draw_frequency_hover(frame, bounds, top, height, track, cursor_position);
                }
            }
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl canvas::Program<SmithChartMessage> for SmithChartCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = canvas::Cache::new().draw(renderer, bounds.size(), |frame| {
            let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
            let radius = bounds.width.min(bounds.height) * 0.44;
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::from_rgb8(20, 24, 30));

            let unit = canvas::Path::circle(center, radius);
            frame.stroke(
                &unit,
                canvas::Stroke::default()
                    .with_width(1.5)
                    .with_color(Color::from_rgb8(178, 190, 204)),
            );

            draw_line(
                frame,
                center,
                radius,
                -1.0,
                0.0,
                1.0,
                0.0,
                Color::from_rgb8(82, 92, 105),
                1.0,
            );
            draw_line(
                frame,
                center,
                radius,
                0.0,
                -1.0,
                0.0,
                1.0,
                Color::from_rgb8(61, 69, 80),
                0.8,
            );

            if self.show_grid {
                if self.admittance_chart {
                    draw_admittance_grid(
                        frame,
                        center,
                        radius,
                        &self.resistance_labels,
                        &self.reactance_labels,
                        true,
                    );
                } else {
                    draw_impedance_grid(
                        frame,
                        center,
                        radius,
                        &self.resistance_labels,
                        &self.reactance_labels,
                    );
                }
            }
            if self.show_admittance {
                draw_admittance_grid(
                    frame,
                    center,
                    radius,
                    &self.resistance_labels,
                    &self.reactance_labels,
                    false,
                );
            }
            if self.show_vswr {
                for vswr in self
                    .vswr_circles
                    .iter()
                    .copied()
                    .filter(|value| *value > 1.0)
                {
                    let mag = ((vswr - 1.0) / (vswr + 1.0)) as f32;
                    let circle = canvas::Path::circle(center, radius * mag);
                    frame.stroke(
                        &circle,
                        canvas::Stroke::default()
                            .with_width(0.8)
                            .with_color(Color::from_rgba8(215, 164, 74, 0.45)),
                    );
                    draw_label(
                        frame,
                        format!("VSWR {:.1}", vswr),
                        Point::new(center.x + radius * mag + 4.0, center.y - 3.0),
                        Color::from_rgba8(229, 184, 99, 0.72),
                    );
                }
            }
            if self.show_q {
                for q in self.q_circles.iter().copied().filter(|value| *value > 0.0) {
                    let color = Color::from_rgba8(95, 190, 170, 0.42);
                    if self.admittance_chart {
                        draw_admittance_q_arc(frame, center, radius, q, color);
                        draw_admittance_q_arc(frame, center, radius, -q, color);
                    } else {
                        draw_q_arc(frame, center, radius, q, color);
                        draw_q_arc(frame, center, radius, -q, color);
                    }
                    let (x, y) = if self.admittance_chart {
                        admittance_chart_point(Complex::new(1.0, q))
                    } else {
                        chart_point_from_normalized_impedance(Complex::new(1.0, q))
                    };
                    draw_label(
                        frame,
                        format!("Q {:.1}", q),
                        Point::new(
                            center.x + x as f32 * radius + 4.0,
                            center.y - y as f32 * radius,
                        ),
                        Color::from_rgba8(124, 216, 200, 0.7),
                    );
                }
            }
            for marker in &self.markers {
                let (x, y) = chart_point_from_normalized_impedance(
                    *marker * (1.0 / self.reference_impedance_ohm),
                );
                let position =
                    Point::new(center.x + x as f32 * radius, center.y - y as f32 * radius);
                let dot = canvas::Path::circle(position, 4.0);
                frame.fill(&dot, Color::from_rgb8(244, 218, 118));
                draw_label(
                    frame,
                    format!("{:.1}{:+.1}j", marker.re, marker.im),
                    Point::new(position.x + 6.0, position.y - 6.0),
                    Color::from_rgb8(244, 218, 118),
                );
            }
            for circle in &self.stability_circles {
                draw_chart_circle(
                    frame,
                    center,
                    radius,
                    circle.source_center,
                    circle.source_radius,
                    Color::from_rgba8(255, 138, 92, 0.65),
                );
                draw_chart_circle(
                    frame,
                    center,
                    radius,
                    circle.load_center,
                    circle.load_radius,
                    Color::from_rgba8(122, 167, 255, 0.65),
                );
            }
            for circle in &self.gain_circles {
                let (color, label) = match circle.port {
                    GainCirclePort::Input => (
                        Color::from_rgba8(199, 156, 255, 0.65),
                        format!("{:.1} dB in", circle.target_gain_db),
                    ),
                    GainCirclePort::Output => (
                        Color::from_rgba8(111, 203, 255, 0.65),
                        format!("{:.1} dB out", circle.target_gain_db),
                    ),
                };
                draw_chart_circle(frame, center, radius, circle.center, circle.radius, color);
                draw_chart_circle_label(
                    frame,
                    center,
                    radius,
                    circle.center,
                    circle.radius,
                    label,
                    color,
                );
            }
            for circle in &self.noise_figure_circles {
                let color = Color::from_rgba8(124, 216, 200, 0.68);
                draw_chart_circle(frame, center, radius, circle.center, circle.radius, color);
                draw_chart_circle_label(
                    frame,
                    center,
                    radius,
                    circle.center,
                    circle.radius,
                    format!("{:.1} dB NF", circle.target_noise_figure_db),
                    color,
                );
            }
            for trace in &self.impedance_arc_traces {
                draw_impedance_arc_trace(frame, center, radius, trace);
            }
            draw_frequency_sweep_trace(frame, center, radius, &self.frequency_results);
            for trace in &self.s_parameter_traces {
                draw_s_parameter_trace(frame, center, radius, trace);
            }
            if let Some(point) = self.point {
                let x = center.x + point.re as f32 * radius;
                let y = center.y - point.im as f32 * radius;
                let dot = canvas::Path::circle(Point::new(x, y), 5.0);
                frame.fill(&dot, Color::from_rgb8(238, 92, 82));
            }
            if let Some(position) = cursor.position_in(bounds) {
                let chart_x = (position.x - center.x) / radius;
                let chart_y = (center.y - position.y) / radius;
                let magnitude = chart_x.hypot(chart_y);
                if magnitude <= 1.0 {
                    let cursor_reflection = Complex::new(f64::from(chart_x), f64::from(chart_y));
                    let snapped = nearest_frequency_point(
                        &self.frequency_results,
                        cursor_reflection,
                        self.active_frequency_hz,
                        f64::from(10.0 / radius),
                    );
                    let (hover_point, impedance, reflection_coefficient, frequency_hz) =
                        if let Some(point) = snapped {
                            (
                                Point::new(
                                    center.x + point.reflection_coefficient.re as f32 * radius,
                                    center.y - point.reflection_coefficient.im as f32 * radius,
                                ),
                                point.impedance,
                                point.reflection_coefficient,
                                Some(point.frequency_hz),
                            )
                        } else {
                            (
                                Point::new(position.x, position.y),
                                normalized_impedance_from_chart_point(
                                    f64::from(chart_x),
                                    f64::from(chart_y),
                                ) * self.reference_impedance_ohm,
                                cursor_reflection,
                                None,
                            )
                        };
                    let dot = canvas::Path::circle(hover_point, 3.5);
                    frame.fill(&dot, Color::from_rgb8(244, 218, 118));
                    draw_hover_tooltip(
                        frame,
                        bounds,
                        position,
                        &hover_readout_lines(
                            impedance,
                            reflection_coefficient,
                            frequency_hz,
                            self.frequency_unit,
                        ),
                    );
                }
            }
        });
        vec![geometry]
    }
}

fn nearest_frequency_point(
    points: &[crate::FrequencyPointResult],
    cursor_reflection: Complex,
    active_frequency_hz: f64,
    maximum_distance: f64,
) -> Option<&crate::FrequencyPointResult> {
    let maximum_distance_squared = maximum_distance * maximum_distance;
    points
        .iter()
        .filter_map(|point| {
            let delta = point.reflection_coefficient - cursor_reflection;
            let distance_squared = delta.re.mul_add(delta.re, delta.im * delta.im);
            (distance_squared <= maximum_distance_squared).then_some((
                point,
                distance_squared,
                (point.frequency_hz - active_frequency_hz).abs(),
            ))
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
        })
        .map(|(point, _, _)| point)
}

fn hover_readout_lines(
    impedance: Complex,
    reflection_coefficient: Complex,
    frequency_hz: Option<f64>,
    frequency_unit: ScalarUnit,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(6);
    if let Some(frequency_hz) = frequency_hz {
        lines.push(format!(
            "Frequency = {}",
            format_frequency(frequency_hz, frequency_unit)
        ));
    }
    lines.push(format!(
        "Impedance = {} ({:.2} ∠ {:.2}°)",
        format_hover_complex(impedance, 2),
        clean_zero(impedance.magnitude()),
        clean_zero(impedance.phase_degrees()),
    ));
    lines.push(match impedance.reciprocal() {
        Some(admittance) => format!("Admittance = {}", format_hover_admittance(admittance)),
        None => "Admittance = infinite".to_string(),
    });
    lines.push(format!(
        "Refl-Coeff = {} ({:.3} ∠ {:.1}°)",
        format_hover_complex(reflection_coefficient, 3),
        clean_zero(reflection_coefficient.magnitude()),
        clean_zero(reflection_coefficient.phase_degrees()),
    ));
    let reflection_magnitude = reflection_coefficient.magnitude();
    lines.push(if reflection_magnitude < 1.0 {
        format!(
            "VSWR = {:.2}",
            (1.0 + reflection_magnitude) / (1.0 - reflection_magnitude)
        )
    } else {
        "VSWR = infinite".to_string()
    });
    lines.push(format!(
        "Q-Factor = {}",
        format_hover_quality_factor(quality_factor(impedance))
    ));
    lines
}

fn format_hover_complex(value: Complex, precision: usize) -> String {
    let sign = if value.im < 0.0 { "-" } else { "+" };
    format!(
        "{:.*} {sign} {:.*}j",
        precision,
        clean_zero(value.re),
        precision,
        clean_zero(value.im.abs()),
    )
}

fn format_hover_admittance(value: Complex) -> String {
    let sign = if value.im < 0.0 { "-" } else { "+" };
    format!(
        "{} {sign} {}j",
        format_significant(value.re, 3),
        format_significant(value.im.abs(), 3),
    )
}

fn format_significant(value: f64, significant_digits: i32) -> String {
    if value == 0.0 {
        return format!("{value:.2}");
    }
    let decimal_places = significant_digits - value.abs().log10().floor() as i32 - 1;
    if decimal_places > 0 {
        format!("{:.*}", decimal_places as usize, value)
    } else {
        let scale = 10.0_f64.powi(-decimal_places);
        format!("{:.0}", value / scale) + &"0".repeat((-decimal_places) as usize)
    }
}

fn format_hover_quality_factor(value: f64) -> String {
    if !value.is_finite() {
        return "infinite".to_string();
    }
    if value >= 0.01 {
        return format!("{value:.2}");
    }
    let scientific = format!("{value:.1e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let exponent = exponent.parse::<i32>().unwrap_or_default();
    format!("{mantissa}e{exponent:+}")
}

fn clean_zero(value: f64) -> f64 {
    if value.abs() <= f64::EPSILON {
        0.0
    } else {
        value
    }
}

fn draw_hover_tooltip(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    cursor_position: Point,
    lines: &[String],
) {
    let width = 350.0_f32.min((bounds.width - 8.0).max(120.0));
    let height = 12.0 + lines.len() as f32 * 15.0;
    let mut x = cursor_position.x + 12.0;
    if x + width > bounds.width - 4.0 {
        x = cursor_position.x - width - 12.0;
    }
    let x = x.clamp(4.0, (bounds.width - width - 4.0).max(4.0));
    let y = (cursor_position.y + 12.0)
        .min(bounds.height - height - 4.0)
        .max(4.0);
    let origin = Point::new(x, y);
    let size = iced::Size::new(width, height);
    frame.fill_rectangle(origin, size, Color::from_rgba8(12, 15, 20, 0.94));
    let border = canvas::Path::rectangle(origin, size);
    frame.stroke(
        &border,
        canvas::Stroke::default()
            .with_width(1.0)
            .with_color(Color::from_rgb8(105, 117, 132)),
    );
    for (index, line) in lines.iter().enumerate() {
        frame.fill_text(canvas::Text {
            content: line.clone(),
            position: Point::new(x + 7.0, y + 14.0 + index as f32 * 15.0),
            color: Color::from_rgb8(235, 239, 244),
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        });
    }
}

fn draw_impedance_grid(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    resistance_labels: &[f64],
    reactance_labels: &[f64],
) {
    for resistance in resistance_labels
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
    {
        draw_resistance_circle(
            frame,
            center,
            radius,
            resistance,
            Color::from_rgb8(58, 86, 122),
        );
    }
    for reactance in reactance_labels.iter().copied() {
        draw_reactance_arc(
            frame,
            center,
            radius,
            reactance,
            Color::from_rgb8(82, 65, 120),
        );
    }
    draw_grid_labels(frame, center, radius, resistance_labels, reactance_labels);
}

fn draw_grid_labels(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    resistance_labels: &[f64],
    reactance_labels: &[f64],
) {
    for r in resistance_labels
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
    {
        let (x, y) = chart_point_from_normalized_impedance(Complex::new(r, 0.0));
        draw_label(
            frame,
            format!("{r:.1}"),
            Point::new(
                center.x + x as f32 * radius - 8.0,
                center.y - y as f32 * radius - 14.0,
            ),
            Color::from_rgba8(182, 205, 232, 0.78),
        );
    }
    for x_value in reactance_labels.iter().copied() {
        let (x, y) = chart_point_from_normalized_impedance(Complex::new(1.0, x_value));
        draw_label(
            frame,
            format!("{x_value:+.1}j"),
            Point::new(
                center.x + x as f32 * radius + 4.0,
                center.y - y as f32 * radius - 4.0,
            ),
            Color::from_rgba8(199, 184, 232, 0.78),
        );
    }
}

fn draw_frequency_track(
    frame: &mut canvas::Frame,
    width: f32,
    top: f32,
    height: f32,
    label: &str,
    points: &[(f64, f64)],
    color: Color,
) {
    let left = 58.0;
    let right = (width - 16.0).max(left + 24.0);
    let bottom = top + height;
    let Some((x_min, x_max, y_min, y_max)) = frequency_track_ranges(points) else {
        return;
    };

    draw_screen_line(
        frame,
        Point::new(left, bottom),
        Point::new(right, bottom),
        Color::from_rgb8(82, 92, 105),
        0.8,
    );
    draw_screen_line(
        frame,
        Point::new(left, top),
        Point::new(left, bottom),
        Color::from_rgb8(82, 92, 105),
        0.8,
    );
    let zero_y = plot_point(
        (x_min, 0.0),
        left,
        right,
        top,
        bottom,
        x_min,
        x_max,
        y_min,
        y_max,
    )
    .y;
    draw_screen_line(
        frame,
        Point::new(left, zero_y),
        Point::new(right, zero_y),
        Color::from_rgba8(178, 190, 204, 0.62),
        1.0,
    );
    draw_label(
        frame,
        label.to_string(),
        Point::new(8.0, top + 10.0),
        Color::from_rgb8(198, 207, 218),
    );
    draw_label(
        frame,
        format!("{:.2}", y_max),
        Point::new(8.0, top + 24.0),
        Color::from_rgba8(198, 207, 218, 0.72),
    );
    draw_label(
        frame,
        format!("{:.2}", y_min),
        Point::new(8.0, bottom - 2.0),
        Color::from_rgba8(198, 207, 218, 0.72),
    );
    draw_label(
        frame,
        format!("{:.3} MHz", x_min / 1.0e6),
        Point::new(left, bottom + 10.0),
        Color::from_rgba8(198, 207, 218, 0.72),
    );
    draw_label(
        frame,
        format!("{:.3} MHz", x_max / 1.0e6),
        Point::new((right - 72.0).max(left), bottom + 10.0),
        Color::from_rgba8(198, 207, 218, 0.72),
    );

    let path = canvas::Path::new(|builder| {
        let mut iter = points.iter();
        if let Some(first) = iter.next() {
            builder.move_to(plot_point(
                *first, left, right, top, bottom, x_min, x_max, y_min, y_max,
            ));
        }
        for point in iter {
            builder.line_to(plot_point(
                *point, left, right, top, bottom, x_min, x_max, y_min, y_max,
            ));
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default().with_width(1.6).with_color(color),
    );
}

fn draw_frequency_hover(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    top: f32,
    height: f32,
    track: &PlotTrack,
    cursor_position: Point,
) {
    let left = 58.0;
    let right = (bounds.width - 16.0).max(left + 24.0);
    let bottom = top + height;
    if cursor_position.x < left
        || cursor_position.x > right
        || cursor_position.y < top
        || cursor_position.y > bottom
    {
        return;
    }
    let Some((x_min, x_max, y_min, y_max)) = frequency_track_ranges(&track.points) else {
        return;
    };
    let frequency_hz =
        x_min + (cursor_position.x - left) as f64 / (right - left) as f64 * (x_max - x_min);
    let Some(value) = interpolate_plot_value(&track.points, frequency_hz) else {
        return;
    };
    let hover_point = plot_point(
        (frequency_hz, value),
        left,
        right,
        top,
        bottom,
        x_min,
        x_max,
        y_min,
        y_max,
    );
    draw_screen_line(
        frame,
        Point::new(hover_point.x, top),
        Point::new(hover_point.x, bottom),
        Color::from_rgba8(220, 226, 235, 0.36),
        0.8,
    );
    let dot = canvas::Path::circle(hover_point, 3.5);
    frame.fill(&dot, track.color);
    draw_hover_tooltip(
        frame,
        bounds,
        cursor_position,
        &[
            format!("Frequency = {:.6} MHz", frequency_hz / 1.0e6),
            format!("{} = {value:.6}", track.label),
        ],
    );
}

fn frequency_track_ranges(points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let x_min = 0.0_f64;
    let x_max = points
        .iter()
        .map(|(frequency, _)| *frequency)
        .fold(f64::NEG_INFINITY, f64::max);
    let data_y_min = points
        .iter()
        .map(|(_, value)| *value)
        .fold(f64::INFINITY, f64::min);
    let data_y_max = points
        .iter()
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    if !x_min.is_finite() || !x_max.is_finite() || (x_max - x_min).abs() <= f64::EPSILON {
        return None;
    }
    if !data_y_min.is_finite() || !data_y_max.is_finite() {
        return None;
    }
    let mut y_min = data_y_min.min(0.0);
    let mut y_max = data_y_max.max(0.0);
    if (y_max - y_min).abs() <= f64::EPSILON {
        let pad = if y_max.abs() <= f64::EPSILON {
            1.0
        } else {
            y_max.abs() * 0.1
        };
        y_min -= pad;
        y_max += pad;
    }
    Some((x_min, x_max, y_min, y_max))
}

fn interpolate_plot_value(points: &[(f64, f64)], frequency_hz: f64) -> Option<f64> {
    let first = points.first().copied()?;
    if frequency_hz < first.0 {
        return None;
    }
    if frequency_hz == first.0 {
        return Some(first.1);
    }
    let last = points.last().copied()?;
    if frequency_hz > last.0 {
        return None;
    }
    if frequency_hz == last.0 {
        return Some(last.1);
    }
    points.windows(2).find_map(|pair| {
        let [left, right] = pair else {
            return None;
        };
        if frequency_hz < left.0 || frequency_hz > right.0 {
            return None;
        }
        let span = right.0 - left.0;
        if span.abs() <= f64::EPSILON {
            return Some(right.1);
        }
        let ratio = (frequency_hz - left.0) / span;
        Some(left.1 + (right.1 - left.1) * ratio)
    })
}

fn plot_point(
    point: (f64, f64),
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) -> Point {
    let x = (point.0 - x_min) / (x_max - x_min);
    let y = (point.1 - y_min) / (y_max - y_min);
    Point::new(
        left + x as f32 * (right - left),
        bottom - y as f32 * (bottom - top),
    )
}

fn draw_screen_line(frame: &mut canvas::Frame, start: Point, end: Point, color: Color, width: f32) {
    let path = canvas::Path::line(start, end);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(width)
            .with_color(color),
    );
}

pub(in crate::tool) fn draw_label(
    frame: &mut canvas::Frame,
    content: String,
    position: Point,
    color: Color,
) {
    frame.fill_text(canvas::Text {
        content,
        position,
        color,
        size: iced::Pixels(10.0),
        ..canvas::Text::default()
    });
}

fn draw_chart_circle(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    chart_center: Complex,
    chart_radius: f64,
    color: Color,
) {
    if !chart_radius.is_finite() || chart_radius <= 0.0 {
        return;
    }
    let circle = canvas::Path::circle(
        Point::new(
            center.x + chart_center.re as f32 * radius,
            center.y - chart_center.im as f32 * radius,
        ),
        chart_radius as f32 * radius,
    );
    frame.stroke(
        &circle,
        canvas::Stroke::default().with_width(1.2).with_color(color),
    );
}

fn draw_chart_circle_label(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    chart_center: Complex,
    chart_radius: f64,
    label: String,
    color: Color,
) {
    if !chart_radius.is_finite() || chart_radius <= 0.0 {
        return;
    }
    draw_label(
        frame,
        label,
        Point::new(
            center.x + chart_center.re as f32 * radius,
            center.y - (chart_center.im as f32 + chart_radius as f32) * radius - 4.0,
        ),
        color,
    );
}

fn draw_resistance_circle(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    r: f64,
    color: Color,
) {
    let c = r / (r + 1.0);
    let rad = 1.0 / (r + 1.0);
    let circle = canvas::Path::circle(
        Point::new(center.x + c as f32 * radius, center.y),
        rad as f32 * radius,
    );
    frame.stroke(
        &circle,
        canvas::Stroke::default().with_width(0.7).with_color(color),
    );
}

fn draw_reactance_arc(frame: &mut canvas::Frame, center: Point, radius: f32, x: f64, color: Color) {
    let points = (-200..=200)
        .map(|idx| {
            let r = 10.0_f64.powf(idx as f64 / 80.0);
            chart_point_from_normalized_impedance(Complex::new(r, x))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, color, 0.7);
}

fn draw_q_arc(frame: &mut canvas::Frame, center: Point, radius: f32, q: f64, color: Color) {
    let points = (0..=240)
        .map(|idx| {
            let r = 10.0_f64.powf(-2.0 + idx as f64 / 60.0);
            chart_point_from_normalized_impedance(Complex::new(r, r * q))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, color, 0.8);
}

fn draw_s_parameter_trace(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    trace: &SParameterTrace,
) {
    let points = trace
        .points
        .iter()
        .filter(|point| point.magnitude() <= 1.001)
        .map(|point| (point.re, point.im))
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, trace.color, 1.7);
    if let Some(last) = points.last() {
        draw_label(
            frame,
            trace.label.to_string(),
            Point::new(
                center.x + last.0 as f32 * radius + 5.0,
                center.y - last.1 as f32 * radius - 5.0,
            ),
            trace.color,
        );
    }
}

fn draw_impedance_arc_trace(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    trace: &ImpedanceArcTrace,
) {
    let points = trace
        .points
        .iter()
        .filter(|point| point.magnitude() <= 1.001)
        .map(|point| (point.re, point.im))
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, trace.color, 1.5);
    if let Some(last) = points.last() {
        draw_label(
            frame,
            trace.label.clone(),
            Point::new(
                center.x + last.0 as f32 * radius + 5.0,
                center.y - last.1 as f32 * radius - 5.0,
            ),
            trace.color,
        );
    }
}

fn draw_frequency_sweep_trace(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    frequency_results: &[crate::FrequencyPointResult],
) {
    let points = frequency_results
        .iter()
        .map(|point| {
            (
                point.reflection_coefficient.re,
                point.reflection_coefficient.im,
            )
        })
        .filter(|(x, y)| x.is_finite() && y.is_finite() && x.hypot(*y) <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(
        frame,
        center,
        radius,
        &points,
        Color::from_rgb8(238, 92, 82),
        1.2,
    );
}

pub(in crate::tool) fn draw_polyline(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    points: &[(f64, f64)],
    color: Color,
    width: f32,
) {
    if points.len() < 2 {
        return;
    }
    let path = canvas::Path::new(|builder| {
        let first = points[0];
        builder.move_to(Point::new(
            center.x + first.0 as f32 * radius,
            center.y - first.1 as f32 * radius,
        ));
        for point in &points[1..] {
            builder.line_to(Point::new(
                center.x + point.0 as f32 * radius,
                center.y - point.1 as f32 * radius,
            ));
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(width)
            .with_color(color),
    );
}

fn draw_line(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    width: f32,
) {
    let path = canvas::Path::line(
        Point::new(center.x + x1 * radius, center.y - y1 * radius),
        Point::new(center.x + x2 * radius, center.y - y2 * radius),
    );
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(width)
            .with_color(color),
    );
}
