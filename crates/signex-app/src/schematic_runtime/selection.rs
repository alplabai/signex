use super::*;

/// The box that frames `selected`, the union of the boxes the selection
/// overlay draws; `None` when no selected item is on the sheet.
pub fn selection_bounds(
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    designators: &Designators,
    widths: WireWidths<'_>,
) -> Option<Aabb> {
    selected_aabbs(snapshot, selected, designators, widths)
        .into_iter()
        .flatten()
        .reduce(|a, b| a.union(&b))
}

/// The selection overlay: an outline in the theme's selection colour, no fill.
pub(super) fn selection_overlay_inputs(
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
    widths: WireWidths<'_>,
) -> OverlayInputs {
    let stroke = to_iced(&colors.selection);
    let mut overlays = OverlayInputs::default();

    for bbox in selected_aabbs(snapshot, selected, designators, widths)
        .into_iter()
        .flatten()
    {
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
                stroke_width_mm: screen_px_to_world_mm(
                    signex_types::schematic::SCHEMATIC_SELECTION_MARKER_OUTLINE_PX,
                    transform.scale,
                ) as f32,
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
                fill_color: [0.0; 4],
                stroke_color: Some(to_rgba(stroke)),
                stroke_width_mm: screen_px_to_world_mm(
                    signex_types::schematic::SCHEMATIC_SELECTION_RECT_OUTLINE_PX,
                    transform.scale,
                ) as f32,
            });
        }
    }

    overlays
}

pub fn draw_selection_overlay(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
    widths: WireWidths<'_>,
) {
    let overlays =
        selection_overlay_inputs(snapshot, selected, transform, colors, designators, widths);
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Wire;
    use signex_types::theme::{ThemeId, canvas_colors};

    fn overlay_for(scale: f32, colors: &CanvasColors) -> OverlayInputs {
        let wire = Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(4.0, 0.0),
            ..Wire::default()
        };
        let selected = [SelectedItem::new(wire.uuid, SelectedKind::Wire)];
        let sheet = SchematicSheet {
            wires: vec![wire],
            ..SchematicSheet::default()
        };
        let transform = ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale,
            device_px_per_logical_px: 1.0,
        };
        selection_overlay_inputs(
            &sheet,
            &selected,
            &transform,
            colors,
            &Designators::new(),
            WireWidths::default(),
        )
    }

    #[test]
    fn selection_is_an_outline_in_the_theme_selection_colour() {
        for &id in ThemeId::BUILTINS {
            let colors = canvas_colors(id);
            let want = to_rgba(to_iced(&colors.selection));
            // A box at 20 px/mm, and a marker-sized one at 0.1 px/mm.
            for scale in [20.0, 0.1] {
                let overlays = overlay_for(scale, &colors);
                assert!(
                    !overlays.ghost_polygons.is_empty() || !overlays.snap_circles.is_empty(),
                    "{id:?} @ {scale}: nothing drawn"
                );
                for poly in &overlays.ghost_polygons {
                    assert_eq!(poly.fill_color[3], 0.0, "{id:?}: selection box is filled");
                    assert_eq!(poly.stroke_color, Some(want), "{id:?}");
                }
                for circle in &overlays.snap_circles {
                    assert!(circle.stroke_width_mm > 0.0, "{id:?}: filled marker disc");
                    assert_eq!(circle.color, want, "{id:?}");
                }
            }
        }
    }
}
