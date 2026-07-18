use super::*;

pub fn draw_selection_overlay(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
) {
    let stroke = Color::from_rgba(0.95, 0.95, 1.0, 0.95);
    let fill = Color::from_rgba(0.65, 0.72, 1.0, 0.12);
    let mut overlays = OverlayInputs::default();

    for item in selected {
        if let Some(bbox) = item_aabb(snapshot, item) {
            let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
            let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
            let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());

            if size.width <= signex_types::schematic::SCHEMATIC_RENDER_SELECTION_MARKER_THRESHOLD_PX
                && size.height
                    <= signex_types::schematic::SCHEMATIC_RENDER_SELECTION_MARKER_THRESHOLD_PX
            {
                let center = [
                    ((bbox.min_x + bbox.max_x) * 0.5) as f32,
                    ((bbox.min_y + bbox.max_y) * 0.5) as f32,
                ];
                overlays.snap_circles.push(OverlayCircleInput {
                    center,
                    radius_mm: screen_px_to_world_mm(
                        signex_types::schematic::SCHEMATIC_RENDER_SELECTION_MARKER_RADIUS_PX,
                        transform.scale,
                    ) as f32,
                    stroke_width_mm: 0.0,
                    color: to_rgba(fill),
                });
                overlays.snap_circles.push(OverlayCircleInput {
                    center,
                    radius_mm: screen_px_to_world_mm(
                        signex_types::schematic::SCHEMATIC_RENDER_SELECTION_MARKER_RADIUS_PX,
                        transform.scale,
                    ) as f32,
                    stroke_width_mm: stroke_world_mm(
                        signex_types::schematic::SCHEMATIC_RENDER_SELECTION_MARKER_STROKE_PX,
                        transform.scale,
                    ),
                    color: to_rgba(stroke),
                });
            } else {
                overlays.ghost_polygons.push(OverlayPolygonInput {
                    vertices: vec![
                        [bbox.min_x as f32, bbox.min_y as f32],
                        [bbox.max_x as f32, bbox.min_y as f32],
                        [bbox.max_x as f32, bbox.max_y as f32],
                        [bbox.min_x as f32, bbox.max_y as f32],
                    ],
                    fill_color: to_rgba(fill),
                    stroke_color: Some(to_rgba(stroke)),
                    stroke_width_mm: stroke_world_mm(
                        signex_types::schematic::SCHEMATIC_RENDER_SELECTION_RECT_STROKE_PX,
                        transform.scale,
                    ),
                });
            }
        }
    }

    if overlays.preview_lines.is_empty()
        && overlays.ghost_polygons.is_empty()
        && overlays.lasso_lines.is_empty()
        && overlays.snap_circles.is_empty()
    {
        return;
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
