use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GerberMeasurement {
    pub start: signex_gerber::Point,
    pub end: Option<signex_gerber::Point>,
}

impl GerberMeasurement {
    pub fn delta(self) -> Option<signex_gerber::Point> {
        self.end.map(|end| signex_gerber::Point {
            x: end.x - self.start.x,
            y: end.y - self.start.y,
        })
    }

    pub fn distance(self) -> Option<f64> {
        self.delta().map(|delta| delta.x.hypot(delta.y))
    }

    pub fn angle_radians(self) -> Option<f64> {
        self.delta().map(|delta| delta.y.atan2(delta.x))
    }

    pub fn angle_degrees(self) -> Option<f64> {
        self.angle_radians().map(f64::to_degrees)
    }
}

impl GerberViewerState {
    pub fn measurement_active(&self) -> bool {
        self.measurement_active
    }

    pub fn measurement(&self) -> Option<GerberMeasurement> {
        self.measurement
    }

    pub fn toggle_measurement(&mut self) {
        if self.measurement_active {
            self.clear_measurement_state();
            self.status = "Measurement cancelled.".into();
        } else {
            self.activate_measurement_tool();
            return;
        }
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }

    pub fn activate_measurement_tool(&mut self) {
        self.measurement_active = true;
        self.measurement = None;
        self.zoom_selection_active = false;
        self.status = "Measurement tool active. Click and drag between two points.".into();
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }

    pub fn begin_measurement(&mut self, point: signex_gerber::Point) {
        if !self.measurement_active || !point.x.is_finite() || !point.y.is_finite() {
            return;
        }
        self.measurement = Some(GerberMeasurement {
            start: point,
            end: Some(point),
        });
        self.status = "Drag to the second measurement point.".into();
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }

    pub fn update_measurement(&mut self, point: signex_gerber::Point) {
        if !self.measurement_active || !point.x.is_finite() || !point.y.is_finite() {
            return;
        }
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.end = Some(point);
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn complete_measurement(&mut self, point: signex_gerber::Point) {
        self.update_measurement(point);
        if self.measurement.is_some() {
            self.status = self
                .measurement_summary()
                .unwrap_or_else(|| "Measurement complete.".into());
        }
    }

    pub fn capture_measurement_point(&mut self, point: signex_gerber::Point) {
        if !self.measurement_active || !point.x.is_finite() || !point.y.is_finite() {
            return;
        }
        match self.measurement {
            None | Some(GerberMeasurement { end: Some(_), .. }) => {
                self.measurement = Some(GerberMeasurement {
                    start: point,
                    end: None,
                });
                self.status = "Select the second measurement point.".into();
            }
            Some(mut measurement) => {
                measurement.end = Some(point);
                self.measurement = Some(measurement);
                self.measurement_active = false;
                self.status = self
                    .measurement_summary()
                    .unwrap_or_else(|| "Measurement complete.".into());
            }
        }
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }

    pub fn reset_measurement(&mut self) {
        self.clear_measurement_state();
        self.status = "Measurement reset.".into();
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
    }

    pub(super) fn clear_measurement_state(&mut self) {
        self.measurement_active = false;
        self.measurement = None;
    }

    pub fn measurement_summary(&self) -> Option<String> {
        format_measurement(
            self.measurement?,
            self.display_unit,
            &self.decimal_separator,
        )
    }

    pub fn measurement_annotation(&self) -> Option<String> {
        format_measurement_annotation(
            self.measurement?,
            self.display_unit,
            &self.decimal_separator,
        )
    }
}

pub(super) fn format_measurement(
    measurement: GerberMeasurement,
    unit: GerberDisplayUnit,
    decimal_separator: &str,
) -> Option<String> {
    let delta = measurement.delta()?;
    let distance = measurement.distance()?;
    Some(format!(
        "Distance: {} {}  ΔX: {}  ΔY: {}",
        unit.format_value(distance, decimal_separator),
        unit.suffix(),
        unit.format_value(delta.x, decimal_separator),
        unit.format_value(delta.y, decimal_separator),
    ))
}

pub(super) fn format_measurement_annotation(
    measurement: GerberMeasurement,
    unit: GerberDisplayUnit,
    decimal_separator: &str,
) -> Option<String> {
    let delta = measurement.delta()?;
    let distance = measurement.distance()?;
    let angle = measurement.angle_degrees()?;
    let displayed_angle = (angle * 100.0).round() / 100.0;
    let displayed_angle_radians = displayed_angle.to_radians();
    let replace_separator = |value: String| {
        if decimal_separator == "." {
            value
        } else {
            value.replace('.', decimal_separator)
        }
    };
    Some(format!(
        "Δx {} {}  Δy {} {}\nr {} {}  θ {}° ({} rad)",
        unit.format_value(delta.x, decimal_separator),
        unit.suffix(),
        unit.format_value(delta.y, decimal_separator),
        unit.suffix(),
        unit.format_value(distance, decimal_separator),
        unit.suffix(),
        replace_separator(format!("{displayed_angle:.2}")),
        replace_separator(format!("{displayed_angle_radians:.5}")),
    ))
}

pub(super) fn draw_measurement(
    frame: &mut canvas::Frame,
    measurement: GerberMeasurement,
    world_to_screen: &impl Fn(signex_gerber::Point) -> Point,
    color: Color,
    annotation: Option<&str>,
) {
    let start = world_to_screen(measurement.start);
    frame.fill(&canvas::Path::circle(start, 3.5), color);
    let Some(end) = measurement.end.map(world_to_screen) else {
        return;
    };
    frame.fill(&canvas::Path::circle(end, 3.5), color);
    frame.stroke(
        &canvas::Path::line(start, end),
        canvas::Stroke::default().with_color(color).with_width(1.5),
    );
    if let Some(annotation) = annotation {
        let lines = annotation.lines().count().max(1) as f32;
        let longest_line = annotation.lines().map(str::len).max().unwrap_or(1) as f32;
        let size = iced::Size::new(longest_line * 7.2 + 12.0, lines * 16.0 + 8.0);
        let position = Point::new(end.x + 12.0, end.y + 12.0);
        frame.fill_rectangle(position, size, Color::from_rgba(0.04, 0.04, 0.04, 0.88));
        frame.fill_text(canvas::Text {
            content: annotation.into(),
            position: Point::new(position.x + 6.0, position.y + 5.0),
            color,
            size: iced::Pixels(12.0),
            ..canvas::Text::default()
        });
    }
}

#[cfg(test)]
#[path = "../../tests/gerber_viewer/measurement.rs"]
mod gerber_measurement_test_definitions;

#[cfg(test)]
gerber_measurement_test_definitions::gerber_measurement_tests!();
