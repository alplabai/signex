use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcMarker {
    pub x_mm: f64,
    pub y_mm: f64,
    pub severity: ErcSeverity,
    /// Net-level finding: drawn as a ring-and-diamond in the theme's net
    /// colour, unlike the filled severity disc of a point finding.
    pub net_level: bool,
}

/// How much of the part under a point marker still shows through its disc and
/// its halo. The outline carries the severity; the fills stay faint so the
/// symbol the finding points at is never hidden (#1768).
const MARKER_DISC_ALPHA: f32 = 0.22;
const MARKER_HALO_ALPHA: f32 = 0.08;
const _: () = assert!(MARKER_DISC_ALPHA <= 0.3 && MARKER_HALO_ALPHA <= MARKER_DISC_ALPHA);

/// Least contrast (WCAG ratio) a marker's ring and cross keep against the sheet.
const MARKER_MIN_CONTRAST: f32 = 3.0;

fn severity_color(severity: ErcSeverity) -> Color {
    match severity {
        ErcSeverity::Error => Color::from_rgb(0.95, 0.25, 0.25),
        ErcSeverity::Warning => Color::from_rgb(0.98, 0.72, 0.20),
        ErcSeverity::Info => Color::from_rgb(0.30, 0.55, 0.85),
    }
}

/// The ring and cross colour for `severity` on a sheet of `paper` and
/// `background`: the severity colour, pulled toward black on a light sheet and
/// toward white on a dark one until it reads against both (#1768).
fn marker_stroke(severity: ErcSeverity, paper: Color, background: Color) -> Color {
    use crate::styles::tokens::{contrast_ratio, relative_luminance};
    // Pull toward whichever pole sits farther from both sheet colours.
    let toward = if relative_luminance(paper).min(relative_luminance(background)) > 0.5 {
        0.0
    } else if relative_luminance(paper).max(relative_luminance(background)) < 0.5 {
        1.0
    } else if relative_luminance(paper) > 0.5 {
        0.0
    } else {
        1.0
    };
    let mut color = severity_color(severity);
    for _ in 0..20 {
        if contrast_ratio(color, paper).min(contrast_ratio(color, background))
            >= MARKER_MIN_CONTRAST
        {
            break;
        }
        color = Color::from_rgb(
            color.r + (toward - color.r) * 0.1,
            color.g + (toward - color.g) * 0.1,
            color.b + (toward - color.b) * 0.1,
        );
    }
    color
}

pub fn draw_erc_markers(
    frame: &mut canvas::Frame,
    markers: &[ErcMarker],
    transform: &ScreenTransform,
    colors: &CanvasColors,
) {
    if markers.is_empty() {
        return;
    }

    let mut overlays = OverlayInputs::default();
    for marker in markers {
        if marker.net_level {
            push_net_marker(&mut overlays, marker, transform, colors);
            continue;
        }
        let stroke = marker_stroke(
            marker.severity,
            to_iced(&colors.paper),
            to_iced(&colors.background),
        );
        let fill = Color {
            a: MARKER_DISC_ALPHA,
            ..stroke
        };

        let center = [marker.x_mm as f32, marker.y_mm as f32];
        overlays.snap_circles.push(OverlayCircleInput {
            center,
            radius_mm: screen_px_to_world_mm(16.0, transform.scale) as f32,
            stroke_width_mm: 0.0,
            color: [fill.r, fill.g, fill.b, MARKER_HALO_ALPHA],
        });
        overlays.snap_circles.push(OverlayCircleInput {
            center,
            radius_mm: screen_px_to_world_mm(7.0, transform.scale) as f32,
            stroke_width_mm: 0.0,
            color: to_rgba(fill),
        });
        overlays.snap_circles.push(OverlayCircleInput {
            center,
            radius_mm: screen_px_to_world_mm(7.0, transform.scale) as f32,
            stroke_width_mm: screen_px_to_world_mm(2.0, transform.scale) as f32,
            color: to_rgba(stroke),
        });

        let cross_len = screen_px_to_world_mm(4.0, transform.scale) as f32;
        let cx = marker.x_mm as f32;
        let cy = marker.y_mm as f32;
        overlays.preview_lines.push(OverlayLineInput {
            p0: [cx - cross_len, cy - cross_len],
            p1: [cx + cross_len, cy + cross_len],
            width_mm: screen_px_to_world_mm(1.5, transform.scale) as f32,
            color: to_rgba(stroke),
        });
        overlays.preview_lines.push(OverlayLineInput {
            p0: [cx - cross_len, cy + cross_len],
            p1: [cx + cross_len, cy - cross_len],
            width_mm: screen_px_to_world_mm(1.5, transform.scale) as f32,
            color: to_rgba(stroke),
        });
    }

    let snapshot = RendererSnapshot {
        wires: Vec::new(),
        junctions: Vec::new(),
        arcs: Vec::new(),
        polygons: Vec::new(),
        labels: Vec::new(),
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts: Vec::new(),
        overlays,
        erc_markers: Vec::new(),
        drawings: Vec::new(),
        drawings_front: Vec::new(),
        wire_color_overrides: HashMap::new(),
        wire_line_styles: HashMap::new(),
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::OVERLAY,
        transform,
    );
}

/// The net-level marker: an open ring with a diamond around it, in the
/// theme's net-label colour. Severity does not recolour it, so it reads as
/// "this whole net" and cannot be mistaken for a point marker.
fn push_net_marker(
    overlays: &mut OverlayInputs,
    marker: &ErcMarker,
    transform: &ScreenTransform,
    colors: &CanvasColors,
) {
    let color = to_rgba(to_iced(&colors.net_label));
    let (cx, cy) = (marker.x_mm as f32, marker.y_mm as f32);
    overlays.snap_circles.push(OverlayCircleInput {
        center: [cx, cy],
        radius_mm: screen_px_to_world_mm(9.0, transform.scale) as f32,
        stroke_width_mm: screen_px_to_world_mm(2.0, transform.scale) as f32,
        color,
    });
    let r = screen_px_to_world_mm(14.0, transform.scale) as f32;
    let width_mm = screen_px_to_world_mm(2.0, transform.scale) as f32;
    let corners = [[cx, cy - r], [cx + r, cy], [cx, cy + r], [cx - r, cy]];
    for i in 0..4 {
        overlays.preview_lines.push(OverlayLineInput {
            p0: corners[i],
            p1: corners[(i + 1) % 4],
            width_mm,
            color,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::tokens::contrast_ratio;
    use signex_types::theme::{ThemeId, canvas_colors};

    #[test]
    fn a_light_paper_on_a_dark_background_still_reads_on_the_paper() {
        let (light, dark) = (iced::Color::WHITE, iced::Color::BLACK);
        for severity in [ErcSeverity::Error, ErcSeverity::Warning, ErcSeverity::Info] {
            let stroke = marker_stroke(severity, light, dark);
            assert!(
                contrast_ratio(stroke, light) >= MARKER_MIN_CONTRAST,
                "{severity:?}"
            );
        }
    }

    #[test]
    fn ring_and_cross_read_on_every_built_in_sheet() {
        let severities = [ErcSeverity::Error, ErcSeverity::Warning, ErcSeverity::Info];
        for &id in ThemeId::BUILTINS {
            let colors = canvas_colors(id);
            let (paper, background) = (to_iced(&colors.paper), to_iced(&colors.background));
            for severity in severities {
                let stroke = marker_stroke(severity, paper, background);
                for sheet in [paper, background] {
                    let ratio = contrast_ratio(stroke, sheet);
                    assert!(
                        ratio >= MARKER_MIN_CONTRAST,
                        "{severity:?} on {id:?}: {ratio:.2}:1"
                    );
                }
            }
        }
    }
}
