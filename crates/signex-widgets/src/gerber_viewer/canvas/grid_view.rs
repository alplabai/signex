use super::*;

pub(in crate::gerber_viewer) fn crosshair_segments(
    bounds: Rectangle,
    position: Point,
    mode: GerberCrosshairMode,
) -> Vec<(Point, Point)> {
    match mode {
        GerberCrosshairMode::None => Vec::new(),
        GerberCrosshairMode::Short => vec![
            (
                Point::new(position.x - 8.0, position.y),
                Point::new(position.x + 8.0, position.y),
            ),
            (
                Point::new(position.x, position.y - 8.0),
                Point::new(position.x, position.y + 8.0),
            ),
        ],
        GerberCrosshairMode::Full => vec![
            (
                Point::new(0.0, position.y),
                Point::new(bounds.width, position.y),
            ),
            (
                Point::new(position.x, 0.0),
                Point::new(position.x, bounds.height),
            ),
        ],
    }
}

#[cfg(test)]
pub(in crate::gerber_viewer) fn full_window_crosshair_segments(
    bounds: Rectangle,
    position: Point,
) -> [(Point, Point); 2] {
    let segments = crosshair_segments(bounds, position, GerberCrosshairMode::Full);
    [segments[0], segments[1]]
}

pub(in crate::gerber_viewer) fn draw_crosshair(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    position: Point,
    color: Color,
    mode: GerberCrosshairMode,
) {
    let color = Color { a: 0.72, ..color };
    for (start, end) in crosshair_segments(bounds, position, mode) {
        frame.stroke(
            &canvas::Path::line(start, end),
            canvas::Stroke::default().with_color(color).with_width(1.0),
        );
    }
}

pub(in crate::gerber_viewer) fn draw_grid(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    color: Color,
    grid_size: &GridSizePreset,
    grid_style: GerberGridStyle,
    pixels_per_millimetre: f32,
    origin: Point,
) {
    let dot_color = grid_render_color(color);
    let x_spacing = visible_grid_spacing(grid_size.x_millimetres() as f32 * pixels_per_millimetre);
    let y_spacing = visible_grid_spacing(grid_size.y_millimetres() as f32 * pixels_per_millimetre);

    match (x_spacing, y_spacing) {
        (Some(x_spacing), Some(y_spacing)) => match grid_style {
            GerberGridStyle::Lines => {
                let mut x = origin.x.rem_euclid(x_spacing);
                while x <= bounds.width {
                    frame.stroke(
                        &canvas::Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                        canvas::Stroke::default()
                            .with_color(dot_color)
                            .with_width(1.0),
                    );
                    x += x_spacing;
                }
                let mut y = origin.y.rem_euclid(y_spacing);
                while y <= bounds.height {
                    frame.stroke(
                        &canvas::Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                        canvas::Stroke::default()
                            .with_color(dot_color)
                            .with_width(1.0),
                    );
                    y += y_spacing;
                }
            }
            GerberGridStyle::Dots | GerberGridStyle::SmallCrosses => {
                let mut x = origin.x.rem_euclid(x_spacing);
                while x <= bounds.width {
                    let mut y = origin.y.rem_euclid(y_spacing);
                    while y <= bounds.height {
                        if grid_style == GerberGridStyle::SmallCrosses {
                            frame.stroke(
                                &canvas::Path::line(Point::new(x - 2.0, y), Point::new(x + 2.0, y)),
                                canvas::Stroke::default()
                                    .with_color(dot_color)
                                    .with_width(1.0),
                            );
                            frame.stroke(
                                &canvas::Path::line(Point::new(x, y - 2.0), Point::new(x, y + 2.0)),
                                canvas::Stroke::default()
                                    .with_color(dot_color)
                                    .with_width(1.0),
                            );
                        } else {
                            frame.fill(
                                &canvas::Path::circle(Point::new(x, y), GRID_DOT_RADIUS_PIXELS),
                                dot_color,
                            );
                        }
                        y += y_spacing;
                    }
                    x += x_spacing;
                }
            }
        },
        (Some(x_spacing), None) => {
            let mut x = origin.x.rem_euclid(x_spacing);
            while x <= bounds.width {
                frame.stroke(
                    &canvas::Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                    canvas::Stroke::default()
                        .with_color(dot_color)
                        .with_width(1.0),
                );
                x += x_spacing;
            }
        }
        (None, Some(y_spacing)) => {
            let mut y = origin.y.rem_euclid(y_spacing);
            while y <= bounds.height {
                frame.stroke(
                    &canvas::Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                    canvas::Stroke::default()
                        .with_color(dot_color)
                        .with_width(1.0),
                );
                y += y_spacing;
            }
        }
        (None, None) => {}
    }
}

const GRID_OPACITY: f32 = 0.72;
const GRID_DOT_RADIUS_PIXELS: f32 = 1.0;

pub(in crate::gerber_viewer) fn grid_render_color(color: Color) -> Color {
    Color {
        a: color.a * GRID_OPACITY,
        ..color
    }
}

pub(in crate::gerber_viewer) fn visible_grid_spacing(spacing: f32) -> Option<f32> {
    const MINIMUM_GRID_SPACING_PIXELS: f32 = 10.0;

    if !spacing.is_finite() || spacing <= 0.0 {
        return None;
    }
    if spacing >= MINIMUM_GRID_SPACING_PIXELS {
        Some(spacing)
    } else {
        Some(spacing * (MINIMUM_GRID_SPACING_PIXELS / spacing).ceil().max(1.0))
    }
}
