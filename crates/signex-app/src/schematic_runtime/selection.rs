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

/// The box of each `selected` item, aligned with `selected`; `None` for an
/// item the sheet no longer holds.
pub fn selection_boxes(
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    designators: &Designators,
    widths: WireWidths<'_>,
) -> Vec<Option<Aabb>> {
    selected_aabbs(snapshot, selected, designators, widths)
}

/// Selection halo sizes in screen px (Graphite): box padding, corner radius
/// and outline width.
const HALO_PAD_PX: f32 = 6.0;
const HALO_RADIUS_PX: f32 = 7.0;
const HALO_OUTLINE_PX: f32 = 1.5;

/// The selection overlay: an outline in the theme's selection colour, no
/// fill. With `graphite` a box is a padded, rounded, dashed accent halo on a
/// soft accent fill instead.
pub(super) fn selection_overlay_inputs(
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    graphite: Option<&crate::canvas::GraphiteCanvasInk>,
    designators: &Designators,
    widths: WireWidths<'_>,
) -> OverlayInputs {
    let boxes: Vec<Aabb> = selected_aabbs(snapshot, selected, designators, widths)
        .into_iter()
        .flatten()
        .collect();
    overlay_inputs_for_boxes(&boxes, transform, colors, graphite)
}

/// [`selection_overlay_inputs`] for boxes already found.
pub(crate) fn overlay_inputs_for_boxes(
    boxes: &[Aabb],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    graphite: Option<&crate::canvas::GraphiteCanvasInk>,
) -> OverlayInputs {
    let stroke = to_iced(&colors.selection);
    let mut overlays = OverlayInputs::default();

    for bbox in boxes.iter().copied() {
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
        } else if let Some(ink) = graphite {
            let mm = |px: f32| screen_px_to_world_mm(px, transform.scale) as f32;
            overlays
                .halos
                .push(signex_renderer::schematic::OverlayHaloInput {
                    min: [bbox.min_x as f32, bbox.min_y as f32],
                    max: [bbox.max_x as f32, bbox.max_y as f32],
                    pad_mm: mm(HALO_PAD_PX),
                    radius_mm: mm(HALO_RADIUS_PX),
                    stroke_width_mm: mm(HALO_OUTLINE_PX),
                    fill_color: to_rgba(Color {
                        a: crate::canvas::HALO_FILL_ALPHA,
                        ..ink.accent_soft
                    }),
                    stroke_color: to_rgba(ink.accent),
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

#[allow(clippy::too_many_arguments)]
pub fn draw_selection_overlay(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    graphite: Option<&crate::canvas::GraphiteCanvasInk>,
    designators: &Designators,
    widths: WireWidths<'_>,
) {
    let overlays = selection_overlay_inputs(
        snapshot,
        selected,
        transform,
        colors,
        graphite,
        designators,
        widths,
    );
    draw_overlay_inputs(frame, overlays, transform, colors);
}

/// Draw the selection overlay of `boxes`.
pub fn draw_selection_boxes(
    frame: &mut canvas::Frame,
    boxes: &[Aabb],
    transform: &ScreenTransform,
    colors: &CanvasColors,
    graphite: Option<&crate::canvas::GraphiteCanvasInk>,
) {
    let overlays = overlay_inputs_for_boxes(boxes, transform, colors, graphite);
    draw_overlay_inputs(frame, overlays, transform, colors);
}

fn draw_overlay_inputs(
    frame: &mut canvas::Frame,
    overlays: OverlayInputs,
    transform: &ScreenTransform,
    colors: &CanvasColors,
) {
    if overlays.preview_lines.is_empty()
        && overlays.halos.is_empty()
        && overlays.ghost_polygons.is_empty()
        && overlays.lasso_lines.is_empty()
        && overlays.snap_circles.is_empty()
    {
        return;
    }

    let snapshot = RendererSnapshot {
        overlays,
        ..Default::default()
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

    fn overlay_with(
        scale: f32,
        colors: &CanvasColors,
        graphite: Option<&crate::canvas::GraphiteCanvasInk>,
    ) -> OverlayInputs {
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
            graphite,
            &Designators::new(),
            WireWidths::default(),
        )
    }

    fn overlay_for(scale: f32, colors: &CanvasColors) -> OverlayInputs {
        overlay_with(scale, colors, None)
    }

    #[test]
    fn graphite_selects_with_a_halo_and_other_themes_keep_the_outline() {
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let ink = crate::canvas::GraphiteCanvasInk::from_tokens(&tokens).expect("graphite");
        let colors = canvas_colors(ThemeId::GraphiteLight);
        let overlays = overlay_with(20.0, &colors, Some(&ink));
        assert_eq!(overlays.halos.len(), 1);
        assert!(overlays.ghost_polygons.is_empty());
        let halo = &overlays.halos[0];
        assert_eq!(halo.stroke_color, to_rgba(ink.accent));
        assert_eq!(halo.fill_color[3], crate::canvas::HALO_FILL_ALPHA);
        // 6 px of padding at 20 px/mm.
        assert!((halo.pad_mm * 20.0 - HALO_PAD_PX).abs() < 1e-3);
        // Without the inks the same selection is the legacy outline.
        let legacy = overlay_with(20.0, &colors, None);
        assert!(legacy.halos.is_empty() && !legacy.ghost_polygons.is_empty());
    }

    #[test]
    fn a_marker_sized_selection_stays_a_ring_in_graphite() {
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let ink = crate::canvas::GraphiteCanvasInk::from_tokens(&tokens).expect("graphite");
        let colors = canvas_colors(ThemeId::GraphiteLight);
        let overlays = overlay_with(0.1, &colors, Some(&ink));
        assert!(overlays.halos.is_empty());
        assert!(!overlays.snap_circles.is_empty());
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
