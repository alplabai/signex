//! Per-primitive scene emitters for the schematic renderer.

use super::*;

pub(super) fn resolve_wire_color(
    wire: &WireInput,
    snapshot: &SchematicSnapshot,
    theme: &ResolvedTheme,
) -> [f32; 4] {
    snapshot
        .wire_color_overrides
        .get(&wire.id)
        .copied()
        .or(wire.explicit_color)
        .unwrap_or(theme.color(ColorSlot::Wire))
}

pub(super) fn emit_wires(snapshot: &SchematicSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.lines.clear();
    scene.lines.reserve(snapshot.wires.len());

    for wire in &snapshot.wires {
        scene.lines.push(LineSegment {
            p0: wire.p0,
            p1: wire.p1,
            width: wire.width_mm,
            color: resolve_wire_color(wire, snapshot, theme),
            style: 0,
            _pad: 0,
        });
    }
}

pub(super) fn emit_junctions(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.circles.clear();
    scene.circles.reserve(snapshot.junctions.len());

    for junction in &snapshot.junctions {
        scene.circles.push(Circle {
            center: junction.center,
            radius: junction.radius_mm,
            stroke_width: 0.0,
            color: junction.color,
        });
    }
}

pub(super) fn emit_arcs(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.arcs.clear();
    scene.arcs.reserve(snapshot.arcs.len());

    for arc in &snapshot.arcs {
        scene.arcs.push(Arc {
            center: arc.center,
            radius: arc.radius_mm,
            start_angle: arc.start_angle_rad,
            end_angle: arc.end_angle_rad,
            width: arc.width_mm,
            color: arc.color,
            _pad: [0.0; 3],
        });
    }
}

pub(super) fn emit_polygons(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.polygons.clear();
    scene.polygons.reserve(snapshot.polygons.len());

    for polygon in &snapshot.polygons {
        scene.polygons.push(GpuPolygon {
            vertices: polygon.vertices.clone(),
            fill_color: polygon.fill_color,
            stroke_color: polygon.stroke_color,
            stroke_width: polygon.stroke_width_mm,
        });
    }
}

pub(super) fn emit_text_bucket(texts: &[TextInput], output: &mut Vec<TextItem>) {
    for text in texts {
        output.push(TextItem {
            content: text.content.clone(),
            position: text.position,
            size_mm: text.size_mm,
            color: text.color,
            bold: text.bold,
            italic: text.italic,
            rotation: text.rotation_rad,
            h_align: to_text_h_align(text.h_align),
            v_align: to_text_v_align(text.v_align),
        });
    }
}

pub(super) fn to_text_h_align(h_align: HAlign) -> TextHAlign {
    match h_align {
        HAlign::Left => TextHAlign::Left,
        HAlign::Center => TextHAlign::Center,
        HAlign::Right => TextHAlign::Right,
    }
}

pub(super) fn to_text_v_align(v_align: VAlign) -> TextVAlign {
    match v_align {
        VAlign::Top => TextVAlign::Top,
        VAlign::Center => TextVAlign::Center,
        VAlign::Bottom => TextVAlign::Bottom,
    }
}

pub(super) fn emit_texts(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.texts.clear();
    scene.texts.reserve(
        snapshot.labels.len()
            + snapshot.pin_texts.len()
            + snapshot.reference_value_texts.len()
            + snapshot.parameter_texts.len(),
    );

    emit_text_bucket(&snapshot.labels, &mut scene.texts);
    emit_text_bucket(&snapshot.pin_texts, &mut scene.texts);
    emit_text_bucket(&snapshot.reference_value_texts, &mut scene.texts);
    emit_text_bucket(&snapshot.parameter_texts, &mut scene.texts);
}

pub(super) fn emit_overlay_line_bucket(lines: &[OverlayLineInput], output: &mut Vec<LineSegment>) {
    for line in lines {
        output.push(LineSegment {
            p0: line.p0,
            p1: line.p1,
            width: line.width_mm,
            color: line.color,
            style: 0,
            _pad: 0,
        });
    }
}

pub(super) fn emit_overlay_circle_bucket(circles: &[OverlayCircleInput], output: &mut Vec<Circle>) {
    for circle in circles {
        output.push(Circle {
            center: circle.center,
            radius: circle.radius_mm,
            stroke_width: circle.stroke_width_mm,
            color: circle.color,
        });
    }
}

pub(super) fn emit_overlay_polygon_bucket(polygons: &[OverlayPolygonInput], output: &mut Vec<GpuPolygon>) {
    for polygon in polygons {
        output.push(GpuPolygon {
            vertices: polygon.vertices.clone(),
            fill_color: polygon.fill_color,
            stroke_color: polygon.stroke_color,
            stroke_width: polygon.stroke_width_mm,
        });
    }
}

pub(super) fn emit_overlays(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.overlay_lines.clear();
    scene.overlay_circles.clear();
    scene.overlay_polygons.clear();

    scene
        .overlay_lines
        .reserve(snapshot.overlays.preview_lines.len() + snapshot.overlays.lasso_lines.len());
    scene
        .overlay_circles
        .reserve(snapshot.overlays.snap_circles.len());
    scene
        .overlay_polygons
        .reserve(snapshot.overlays.ghost_polygons.len());

    emit_overlay_line_bucket(&snapshot.overlays.preview_lines, &mut scene.overlay_lines);
    emit_overlay_polygon_bucket(
        &snapshot.overlays.ghost_polygons,
        &mut scene.overlay_polygons,
    );
    emit_overlay_line_bucket(&snapshot.overlays.lasso_lines, &mut scene.overlay_lines);
    emit_overlay_circle_bucket(&snapshot.overlays.snap_circles, &mut scene.overlay_circles);
}

pub(super) fn erc_style_ref(severity: Severity) -> StyleRef {
    let slot = match severity {
        Severity::Error => ColorSlot::ErcError,
        Severity::Warning => ColorSlot::ErcWarning,
        Severity::Info => ColorSlot::ErcInfo,
    };

    StyleRef::new(slot)
}

pub(super) fn erc_color_from_style(style: StyleRef, theme: &ResolvedTheme) -> [f32; 4] {
    let slot = match style.slot {
        slot if slot == ColorSlot::ErcError as u16 => ColorSlot::ErcError,
        slot if slot == ColorSlot::ErcWarning as u16 => ColorSlot::ErcWarning,
        _ => ColorSlot::ErcInfo,
    };

    let mut color = theme.color(slot);

    color[3] = (color[3] * style.alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

pub(super) fn erc_marker_vertices(severity: Severity, center: [f32; 2], radius_mm: f32) -> Vec<[f32; 2]> {
    let radius = radius_mm.max(0.05);
    let cx = center[0];
    let cy = center[1];

    match severity {
        Severity::Error => {
            let tri_half_width = radius * 0.8660254;
            vec![
                [cx, cy - radius],
                [cx + tri_half_width, cy + radius * 0.5],
                [cx - tri_half_width, cy + radius * 0.5],
            ]
        }
        Severity::Warning => vec![
            [cx, cy - radius],
            [cx + radius, cy],
            [cx, cy + radius],
            [cx - radius, cy],
        ],
        Severity::Info => vec![
            [cx - radius, cy - radius],
            [cx + radius, cy - radius],
            [cx + radius, cy + radius],
            [cx - radius, cy + radius],
        ],
    }
}

pub(super) fn erc_marker_line(marker: &ErcMarkerInput) -> ([f32; 2], [f32; 2]) {
    let radius = marker.radius_mm.max(0.05);
    let half = radius * 0.45;
    let cx = marker.center[0];
    let cy = marker.center[1];

    match marker.severity {
        Severity::Error => ([cx, cy - half], [cx, cy + half]),
        Severity::Warning => ([cx - half, cy + half * 0.25], [cx + half, cy + half * 0.25]),
        Severity::Info => ([cx - half, cy], [cx + half, cy]),
    }
}

pub(super) fn emit_erc_markers(snapshot: &SchematicSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.erc_marker_lines.clear();
    scene.erc_marker_circles.clear();
    scene.erc_marker_polygons.clear();

    scene.erc_marker_lines.reserve(snapshot.erc_markers.len());
    scene.erc_marker_circles.reserve(snapshot.erc_markers.len());
    scene
        .erc_marker_polygons
        .reserve(snapshot.erc_markers.len());

    for marker in &snapshot.erc_markers {
        let style = erc_style_ref(marker.severity);
        let stroke_color = erc_color_from_style(style, theme);
        let fill_color = [
            stroke_color[0],
            stroke_color[1],
            stroke_color[2],
            (stroke_color[3] * 0.28).clamp(0.0, 1.0),
        ];

        scene.erc_marker_polygons.push(GpuPolygon {
            vertices: erc_marker_vertices(marker.severity, marker.center, marker.radius_mm),
            fill_color,
            stroke_color: Some(stroke_color),
            stroke_width: (marker.radius_mm * 0.12).max(0.03),
        });

        scene.erc_marker_circles.push(Circle {
            center: marker.center,
            radius: marker.radius_mm.max(0.05),
            stroke_width: (marker.radius_mm * 0.18).max(0.04),
            color: stroke_color,
        });

        let (p0, p1) = erc_marker_line(marker);
        scene.erc_marker_lines.push(LineSegment {
            p0,
            p1,
            width: (marker.radius_mm * 0.16).max(0.04),
            color: stroke_color,
            style: 0,
            _pad: 0,
        });
    }
}
