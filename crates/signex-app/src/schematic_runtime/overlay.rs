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
}

pub fn draw_erc_markers(
    frame: &mut canvas::Frame,
    markers: &[ErcMarker],
    transform: &ScreenTransform,
) {
    if markers.is_empty() {
        return;
    }

    let mut overlays = OverlayInputs::default();
    for marker in markers {
        let (fill, stroke) = match marker.severity {
            ErcSeverity::Error => (
                Color::from_rgba(0.95, 0.25, 0.25, 0.6),
                Color::from_rgb(0.95, 0.25, 0.25),
            ),
            ErcSeverity::Warning => (
                Color::from_rgba(0.98, 0.72, 0.20, 0.6),
                Color::from_rgb(0.98, 0.72, 0.20),
            ),
            ErcSeverity::Info => (
                Color::from_rgba(0.30, 0.55, 0.85, 0.55),
                Color::from_rgb(0.30, 0.55, 0.85),
            ),
        };

        let center = [marker.x_mm as f32, marker.y_mm as f32];
        overlays.snap_circles.push(OverlayCircleInput {
            center,
            radius_mm: screen_px_to_world_mm(16.0, transform.scale) as f32,
            stroke_width_mm: 0.0,
            color: [fill.r, fill.g, fill.b, 0.18],
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
            color: to_rgba(Color::WHITE),
        });
        overlays.preview_lines.push(OverlayLineInput {
            p0: [cx - cross_len, cy + cross_len],
            p1: [cx + cross_len, cy - cross_len],
            width_mm: screen_px_to_world_mm(1.5, transform.scale) as f32,
            color: to_rgba(Color::WHITE),
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
        wire_color_overrides: HashMap::new(),
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(signex_types::theme::canvas_colors(
            signex_types::theme::ThemeId::Signex,
        )),
        DirtyFlags::OVERLAY,
        transform,
    );
}
