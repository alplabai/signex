//! A small padlock above every locked symbol (#1250), drawn in the
//! selection layer so it follows the camera and never enters the scene
//! cache. The colour is the theme's `value` token.

use super::*;

/// Whether any symbol of `snapshot` is locked, i.e. the layer has a padlock
/// to draw even with nothing selected.
pub fn has_locked_symbol(snapshot: &SchematicRenderSnapshot) -> bool {
    snapshot.symbols.iter().any(|s| s.locked)
}

pub fn draw_lock_indicators(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
) {
    let locked: Vec<SelectedItem> = snapshot
        .symbols
        .iter()
        .filter(|s| s.locked)
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    if locked.is_empty() {
        return;
    }
    let color = to_rgba(to_iced(&colors.value));
    let px = |n: f32| screen_px_to_world_mm(n, transform.scale) as f32;
    let mut overlays = OverlayInputs::default();

    for bbox in selected_aabbs(snapshot, &locked, designators)
        .into_iter()
        .flatten()
    {
        // Top-right corner of the symbol; world y grows downward.
        let (right, top) = (bbox.max_x as f32, bbox.min_y as f32);
        let (left, base) = (right - px(7.0), top - px(5.0));
        overlays.ghost_polygons.push(OverlayPolygonInput {
            vertices: vec![[left, base], [right, base], [right, top], [left, top]],
            fill_color: color,
            stroke_color: None,
            stroke_width_mm: 0.0,
        });
        let (a, b, rim) = (left + px(1.5), right - px(1.5), base - px(3.0));
        for (p0, p1) in [
            ([a, base], [a, rim]),
            ([a, rim], [b, rim]),
            ([b, rim], [b, base]),
        ] {
            overlays.preview_lines.push(OverlayLineInput {
                p0,
                p1,
                width_mm: px(1.2),
                color,
            });
        }
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
