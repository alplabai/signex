mod projected_point;
mod smith_chart_3d_canvas;
mod smith_chart_3d_canvas_state;
mod smith_sphere;
mod smith_sphere_point;
mod sphere_stroke;

pub(super) use smith_chart_3d_canvas::SmithChart3dCanvas;

use projected_point::ProjectedPoint;
use smith_chart_3d_canvas_state::SmithChart3dCanvasState;
use sphere_stroke::SphereStroke;

#[cfg(test)]
mod tests;

use crate::{Complex, chart_point_from_normalized_impedance};
use iced::widget::canvas;
use iced::{Color, Event, Point, Rectangle, Renderer, Theme, mouse};

use smith_sphere::{normalized_impedance_to_smith_sphere, reflection_to_smith_sphere};
use smith_sphere_point::SmithSpherePoint;

use super::{ImpedanceArcTrace, SParameterTrace, SmithChartMessage};

const ROTATION_SENSITIVITY: f32 = 0.012;
const MAX_PITCH: f32 = 1.35;

impl Default for SmithChart3dCanvasState {
    fn default() -> Self {
        Self {
            drag_start: None,
            drag_yaw: 0.0,
            drag_pitch: 0.0,
        }
    }
}

impl canvas::Program<SmithChartMessage> for SmithChart3dCanvas {
    type State = SmithChart3dCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<SmithChartMessage>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = cursor.position_in(bounds)?;
                state.drag_start = Some(position);
                state.drag_yaw = self.yaw;
                state.drag_pitch = self.pitch;
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Some(start) = state.drag_start else {
                    return None;
                };
                let Some(position) = cursor.position_in(bounds) else {
                    return None;
                };
                let dx = position.x - start.x;
                let dy = position.y - start.y;
                Some(
                    canvas::Action::publish(SmithChartMessage::SmithSphereRotationChanged {
                        yaw: state.drag_yaw + dx * ROTATION_SENSITIVITY,
                        pitch: (state.drag_pitch + dy * ROTATION_SENSITIVITY)
                            .clamp(-MAX_PITCH, MAX_PITCH),
                    })
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.drag_start.take().is_some() {
                    Some(canvas::Action::capture())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = canvas::Cache::new().draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::from_rgb8(20, 24, 30));

            let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
            let radius = bounds.width.min(bounds.height) * 0.39;
            let mut strokes = Vec::new();

            add_sphere_outline(&mut strokes, self.yaw, self.pitch, center, radius);
            if self.show_grid {
                add_smith_grid(
                    &mut strokes,
                    &self.resistance_labels,
                    &self.reactance_labels,
                    self.yaw,
                    self.pitch,
                    center,
                    radius,
                );
            }
            add_complex_traces(
                &mut strokes,
                &self.impedance_arc_traces,
                &self.s_parameter_traces,
                self.yaw,
                self.pitch,
                center,
                radius,
            );

            strokes.sort_by(|a, b| a.depth.total_cmp(&b.depth));
            for stroke in &strokes {
                draw_stroke(frame, stroke);
            }

            for marker in &self.markers {
                let normalized = *marker * (1.0 / self.reference_impedance_ohm);
                let (gamma_re, gamma_im) = chart_point_from_normalized_impedance(normalized);
                draw_sphere_dot(
                    frame,
                    reflection_to_smith_sphere(Complex::new(gamma_re, gamma_im)),
                    self.yaw,
                    self.pitch,
                    center,
                    radius,
                    Color::from_rgb8(244, 218, 118),
                    4.0,
                );
            }
            if let Some(point) = self.point {
                draw_sphere_dot(
                    frame,
                    reflection_to_smith_sphere(point),
                    self.yaw,
                    self.pitch,
                    center,
                    radius,
                    Color::from_rgb8(238, 92, 82),
                    5.0,
                );
            }

            draw_label(
                frame,
                "North: positive resistance".to_string(),
                Point::new(12.0, 20.0),
                Color::from_rgba8(198, 207, 218, 0.82),
            );
            draw_label(
                frame,
                "South: negative resistance".to_string(),
                Point::new(12.0, 38.0),
                Color::from_rgba8(198, 207, 218, 0.70),
            );
            draw_label(
                frame,
                "Equator: |Gamma| = 1".to_string(),
                Point::new(12.0, 56.0),
                Color::from_rgba8(198, 207, 218, 0.70),
            );
            draw_label(
                frame,
                "Drag to rotate".to_string(),
                Point::new(12.0, bounds.height - 16.0),
                Color::from_rgba8(198, 207, 218, 0.70),
            );
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_start.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

fn add_sphere_outline(
    strokes: &mut Vec<SphereStroke>,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
) {
    add_latitude(
        strokes,
        0.0,
        yaw,
        pitch,
        center,
        radius,
        Color::from_rgba8(215, 164, 74, 0.76),
        1.4,
    );
    add_meridian(
        strokes,
        0.0,
        yaw,
        pitch,
        center,
        radius,
        Color::from_rgba8(178, 190, 204, 0.55),
        1.0,
    );
    add_meridian(
        strokes,
        std::f32::consts::FRAC_PI_2,
        yaw,
        pitch,
        center,
        radius,
        Color::from_rgba8(178, 190, 204, 0.36),
        0.9,
    );
}

fn add_smith_grid(
    strokes: &mut Vec<SphereStroke>,
    resistance_labels: &[f64],
    reactance_labels: &[f64],
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
) {
    for resistance_sign in [1.0, -1.0] {
        for r in resistance_labels
            .iter()
            .copied()
            .filter(|value| *value > 0.0)
        {
            let points = (-180..=180)
                .map(|index| {
                    let x = f64::from(index) / 18.0;
                    normalized_impedance_to_smith_sphere(Complex::new(resistance_sign * r, x))
                })
                .collect::<Vec<_>>();
            let color = if resistance_sign > 0.0 {
                Color::from_rgba8(82, 132, 190, 0.58)
            } else {
                Color::from_rgba8(190, 104, 104, 0.50)
            };
            add_sphere_polyline(strokes, points, yaw, pitch, center, radius, color, 0.85);
        }
    }
    for resistance_sign in [1.0, -1.0] {
        for x in reactance_labels.iter().copied() {
            let points = (0..=220)
                .map(|index| {
                    let r = resistance_sign * 10.0_f64.powf(-3.0 + f64::from(index) * 6.0 / 220.0);
                    normalized_impedance_to_smith_sphere(Complex::new(r, x))
                })
                .collect::<Vec<_>>();
            let color = if resistance_sign > 0.0 {
                Color::from_rgba8(155, 124, 216, 0.52)
            } else {
                Color::from_rgba8(216, 124, 155, 0.44)
            };
            add_sphere_polyline(strokes, points, yaw, pitch, center, radius, color, 0.8);
        }
    }
}

fn add_complex_traces(
    strokes: &mut Vec<SphereStroke>,
    impedance_arc_traces: &[ImpedanceArcTrace],
    s_parameter_traces: &[SParameterTrace],
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
) {
    for trace in impedance_arc_traces {
        add_sphere_polyline(
            strokes,
            trace
                .points
                .iter()
                .copied()
                .map(reflection_to_smith_sphere)
                .collect(),
            yaw,
            pitch,
            center,
            radius,
            trace.color,
            2.1,
        );
    }
    for trace in s_parameter_traces {
        add_sphere_polyline(
            strokes,
            trace
                .points
                .iter()
                .copied()
                .map(reflection_to_smith_sphere)
                .collect(),
            yaw,
            pitch,
            center,
            radius,
            trace.color,
            1.6,
        );
    }
}

fn add_latitude(
    strokes: &mut Vec<SphereStroke>,
    z: f32,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
    color: Color,
    width: f32,
) {
    let radial = (1.0 - z * z).sqrt();
    let points = (0..=144)
        .map(|index| {
            let t = index as f32 * std::f32::consts::TAU / 144.0;
            SmithSpherePoint::new(
                f64::from(radial * t.cos()),
                f64::from(radial * t.sin()),
                f64::from(z),
            )
        })
        .collect();
    add_sphere_polyline(strokes, points, yaw, pitch, center, radius, color, width);
}

fn add_meridian(
    strokes: &mut Vec<SphereStroke>,
    longitude: f32,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
    color: Color,
    width: f32,
) {
    let points = (-72..=72)
        .map(|index| {
            let latitude = index as f32 * std::f32::consts::FRAC_PI_2 / 72.0;
            let radial = latitude.cos();
            SmithSpherePoint::new(
                f64::from(radial * longitude.cos()),
                f64::from(radial * longitude.sin()),
                f64::from(latitude.sin()),
            )
        })
        .collect();
    add_sphere_polyline(strokes, points, yaw, pitch, center, radius, color, width);
}

fn add_sphere_polyline(
    strokes: &mut Vec<SphereStroke>,
    points: Vec<SmithSpherePoint>,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
    color: Color,
    width: f32,
) {
    if points.len() < 2 {
        return;
    }
    let projected = points
        .into_iter()
        .map(|point| project(point, yaw, pitch, center, radius))
        .collect::<Vec<_>>();
    let depth = projected.iter().map(|point| point.camera_z).sum::<f32>() / projected.len() as f32;
    strokes.push(SphereStroke {
        points: projected,
        color,
        width,
        depth,
    });
}

fn draw_stroke(frame: &mut canvas::Frame, stroke: &SphereStroke) {
    if stroke.points.len() < 2 {
        return;
    }
    let alpha_scale = ((stroke.depth + 1.0) * 0.35 + 0.35).clamp(0.24, 1.0);
    let color = Color {
        a: stroke.color.a * alpha_scale,
        ..stroke.color
    };
    let path = canvas::Path::new(|builder| {
        builder.move_to(stroke.points[0].screen);
        for point in stroke.points.iter().skip(1) {
            builder.line_to(point.screen);
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_width(stroke.width)
            .with_color(color),
    );
}

fn draw_sphere_dot(
    frame: &mut canvas::Frame,
    point: SmithSpherePoint,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
    color: Color,
    dot_radius: f32,
) {
    let projected = project(point, yaw, pitch, center, radius);
    let alpha_scale = ((projected.camera_z + 1.0) * 0.35 + 0.40).clamp(0.30, 1.0);
    let dot = canvas::Path::circle(projected.screen, dot_radius);
    frame.fill(
        &dot,
        Color {
            a: color.a * alpha_scale,
            ..color
        },
    );
}

fn project(
    point: SmithSpherePoint,
    yaw: f32,
    pitch: f32,
    center: Point,
    radius: f32,
) -> ProjectedPoint {
    let x = point.x as f32;
    let y = point.y as f32;
    let z = point.z as f32;

    let yaw_sin = yaw.sin();
    let yaw_cos = yaw.cos();
    let pitch_sin = pitch.sin();
    let pitch_cos = pitch.cos();

    let x_yaw = x * yaw_cos + y * yaw_sin;
    let y_yaw = -x * yaw_sin + y * yaw_cos;
    let y_pitch = y_yaw * pitch_cos - z * pitch_sin;
    let z_pitch = y_yaw * pitch_sin + z * pitch_cos;

    let perspective = 1.0 / (2.45 - z_pitch * 0.35);
    ProjectedPoint {
        screen: Point::new(
            center.x + x_yaw * radius * perspective * 2.1,
            center.y - y_pitch * radius * perspective * 2.1,
        ),
        camera_z: z_pitch,
    }
}

fn draw_label(frame: &mut canvas::Frame, content: String, position: Point, color: Color) {
    frame.fill_text(canvas::Text {
        content,
        position,
        color,
        size: iced::Pixels(12.0),
        ..canvas::Text::default()
    });
}
