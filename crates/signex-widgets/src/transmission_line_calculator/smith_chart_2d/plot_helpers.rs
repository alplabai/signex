use super::*;
use crate::transmission_line_calculator::FrequencyScale;

const EMPTY_FREQUENCY_RANGE_HZ: (f64, f64) = (MINIMUM_FREQUENCY_HZ, 1.0e6);
const EMPTY_VALUE_RANGE: (f64, f64) = (-1.0, 1.0);

/// Draws frequency track into the target drawing surface.
pub(super) fn draw_frequency_track(
    frame: &mut canvas::Frame,
    width: f32,
    top: f32,
    height: f32,
    track: &PlotTrack,
    frequency_scale: FrequencyScale,
) {
    let left = 58.0;
    let right = (width - 16.0).max(left + 24.0);
    let bottom = top + height;
    let Some((x_min, x_max, y_min, y_max)) = frequency_track_ranges(&track.points) else {
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
        frequency_scale,
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
        track.label.clone(),
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
        format_frequency(x_min, ScalarUnit::Hertz),
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
        let mut iter = track.points.iter();
        if let Some(first) = iter.next() {
            builder.move_to(plot_point(
                *first,
                left,
                right,
                top,
                bottom,
                x_min,
                x_max,
                y_min,
                y_max,
                frequency_scale,
            ));
        }
        for point in iter {
            builder.line_to(plot_point(
                *point,
                left,
                right,
                top,
                bottom,
                x_min,
                x_max,
                y_min,
                y_max,
                frequency_scale,
            ));
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(1.6)
            .with_color(track.color),
    );
}

/// Draws frequency hover into the target drawing surface.
pub(super) fn draw_frequency_hover(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    top: f32,
    height: f32,
    track: &PlotTrack,
    cursor_position: Point,
    frequency_scale: FrequencyScale,
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
    let frequency_hz = frequency_scale.frequency_at(
        x_min,
        x_max,
        (cursor_position.x - left) as f64 / (right - left) as f64,
    );
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
        frequency_scale,
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

/// Computes the frequency track display ranges.
pub(super) fn frequency_track_ranges(points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    if points.is_empty() {
        return Some((
            EMPTY_FREQUENCY_RANGE_HZ.0,
            EMPTY_FREQUENCY_RANGE_HZ.1,
            EMPTY_VALUE_RANGE.0,
            EMPTY_VALUE_RANGE.1,
        ));
    }
    let x_min = MINIMUM_FREQUENCY_HZ;
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

/// Interpolates plot value between the available samples.
pub(super) fn interpolate_plot_value(points: &[(f64, f64)], frequency_hz: f64) -> Option<f64> {
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

/// Maps a data sample into the plot's screen rectangle.
pub(super) fn plot_point(
    point: (f64, f64),
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    frequency_scale: FrequencyScale,
) -> Point {
    let x = frequency_scale.normalize(point.0, x_min, x_max);
    let y = (point.1 - y_min) / (y_max - y_min);
    Point::new(
        left + x as f32 * (right - left),
        bottom - y as f32 * (bottom - top),
    )
}

/// Draws screen line into the target drawing surface.
pub(super) fn draw_screen_line(
    frame: &mut canvas::Frame,
    start: Point,
    end: Point,
    color: Color,
    width: f32,
) {
    let path = canvas::Path::line(start, end);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(width)
            .with_color(color),
    );
}

/// Draws label into the target drawing surface.
pub(in crate::transmission_line_calculator::tool) fn draw_label(
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

/// Draws chart circle into the target drawing surface.
pub(super) fn draw_chart_circle(
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

/// Draws chart circle label into the target drawing surface.
pub(super) fn draw_chart_circle_label(
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

/// Draws Q arc into the target drawing surface.
pub(super) fn draw_q_arc(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    q: f64,
    color: Color,
) {
    let points = (0..=240)
        .map(|idx| {
            let r = 10.0_f64.powf(-2.0 + idx as f64 / 60.0);
            chart_point_from_normalized_impedance(Complex::new(r, r * q))
        })
        .filter(|(x, y)| (*x * *x + *y * *y).sqrt() <= 1.001)
        .collect::<Vec<_>>();
    draw_polyline(frame, center, radius, &points, color, 0.8);
}

/// Draws s parameter trace into the target drawing surface.
pub(super) fn draw_s_parameter_trace(
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

/// Draws impedance arc trace into the target drawing surface.
pub(super) fn draw_impedance_arc_trace(
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

/// Draws frequency sweep trace into the target drawing surface.
pub(super) fn draw_frequency_sweep_trace(
    frame: &mut canvas::Frame,
    center: Point,
    radius: f32,
    frequency_results: &[crate::transmission_line_calculator::FrequencyPointResult],
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

/// Draws polyline into the target drawing surface.
pub(in crate::transmission_line_calculator::tool) fn draw_polyline(
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

/// Draws line into the target drawing surface.
pub(super) fn draw_line(
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
