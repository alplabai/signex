//! PCB scene emitters + snapshot geometry helpers.

use super::*;

pub(super) fn trace_from_segment(segment: &Segment) -> TraceInput {
    TraceInput {
        p0: point_to_xy(segment.start),
        p1: point_to_xy(segment.end),
        width_mm: mm_with_floor(segment.width, PCB_DEFAULT_TRACE_WIDTH_MM, PCB_TRACK_MIN_MM),
        net: segment.net,
    }
}

pub(super) fn via_from_board_via(via: &Via) -> ViaInput {
    ViaInput {
        center: point_to_xy(via.position),
        diameter_mm: mm_with_floor(
            via.diameter,
            PCB_DEFAULT_VIA_DIAMETER_MM,
            PCB_VIA_MIN_DIAMETER_MM,
        ),
        drill_mm: mm_with_floor(via.drill, PCB_DEFAULT_VIA_DRILL_MM, PCB_VIA_MIN_DRILL_MM),
        net: via.net,
    }
}

pub(super) fn pad_from_footprint(footprint: &Footprint, pad: &Pad) -> PadInput {
    let local = point_to_xy(pad.position);
    let rotated = rotate_local(local, footprint.rotation as f32);
    let footprint_origin = point_to_xy(footprint.position);

    PadInput {
        center: [
            footprint_origin[0] + rotated[0],
            footprint_origin[1] + rotated[1],
        ],
        size_mm: [
            mm_with_floor(pad.size.x, PCB_DEFAULT_PAD_SIZE_MM, 0.05),
            mm_with_floor(pad.size.y, PCB_DEFAULT_PAD_SIZE_MM, 0.05),
        ],
        shape: pad.shape,
        pad_type: pad.pad_type,
    }
}

pub(super) fn zone_from_board_zone(
    zone: &Zone,
    source_order: usize,
    layer_ranks: &HashMap<String, u16>,
) -> Option<ZonePolygonInput> {
    if zone.outline.len() < 3 {
        return None;
    }

    let vertices = zone
        .outline
        .iter()
        .map(|point| point_to_xy(*point))
        .collect();
    let layer_name = zone.layer.to_ascii_lowercase();
    let layer_rank = layer_ranks.get(&layer_name).copied().unwrap_or(u16::MAX);

    Some(ZonePolygonInput {
        vertices,
        rule_area: is_rule_area_zone(zone),
        net: zone.net,
        priority: zone.priority,
        layer_rank,
        layer_name,
        source_order,
    })
}

pub(super) fn layer_rank_index(board: &PcbBoard) -> HashMap<String, u16> {
    let mut index = HashMap::with_capacity(board.layers.len());

    for layer in &board.layers {
        index.insert(layer.name.to_ascii_lowercase(), u16::from(layer.id));
    }

    index
}

pub(super) fn sort_zone_stack(zones: &mut [ZonePolygonInput]) {
    zones.sort_by(|a, b| {
        a.layer_rank
            .cmp(&b.layer_rank)
            .then_with(|| a.layer_name.cmp(&b.layer_name))
            .then_with(|| zone_layer_top_composite_key(a).cmp(&zone_layer_top_composite_key(b)))
            .then_with(|| a.source_order.cmp(&b.source_order))
    });
}

pub(super) fn zone_layer_top_composite_key(zone: &ZonePolygonInput) -> (u32, u8, u32) {
    // Ascending order is intentional: lower priority and net bucket are painted first,
    // so higher-priority connected zones land on top in the same layer stack.
    (zone.priority, zone_connected_bucket(zone.net), zone.net)
}

pub(super) fn zone_connected_bucket(net: u32) -> u8 {
    if net == 0 { 0 } else { 1 }
}

pub(super) fn is_rule_area_zone(zone: &Zone) -> bool {
    let fill = zone.fill_type.to_ascii_lowercase();
    let layer = zone.layer.to_ascii_lowercase();

    fill.contains("rule") || fill.contains("keepout") || layer.contains("rule")
}

pub(super) fn point_to_xy(point: Point) -> [f32; 2] {
    [point.x as f32, point.y as f32]
}

pub(super) fn mm_with_floor(value: f64, fallback: f64, floor: f64) -> f32 {
    let candidate = if value > 0.0 { value } else { fallback };
    candidate.max(floor) as f32
}

pub(super) fn rotate_local(point: [f32; 2], rotation_deg: f32) -> [f32; 2] {
    let angle = rotation_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    [
        point[0] * cos_a - point[1] * sin_a,
        point[0] * sin_a + point[1] * cos_a,
    ]
}

pub(super) fn rectangle_vertices(center: [f32; 2], size_mm: [f32; 2]) -> Vec<[f32; 2]> {
    let half_w = (size_mm[0] * 0.5).max(0.01);
    let half_h = (size_mm[1] * 0.5).max(0.01);

    vec![
        [center[0] - half_w, center[1] - half_h],
        [center[0] + half_w, center[1] - half_h],
        [center[0] + half_w, center[1] + half_h],
        [center[0] - half_w, center[1] + half_h],
    ]
}

pub(super) fn ellipse_vertices(
    center: [f32; 2],
    radius_xy: [f32; 2],
    segments: usize,
) -> Vec<[f32; 2]> {
    let count = segments.max(8);
    let mut vertices = Vec::with_capacity(count);

    for i in 0..count {
        let t = (i as f32 / count as f32) * std::f32::consts::TAU;
        vertices.push([
            center[0] + radius_xy[0] * t.cos(),
            center[1] + radius_xy[1] * t.sin(),
        ]);
    }

    vertices
}

pub(super) fn pad_vertices(pad: &PadInput) -> Vec<[f32; 2]> {
    let half_w = (pad.size_mm[0] * 0.5).max(0.01);
    let half_h = (pad.size_mm[1] * 0.5).max(0.01);

    match pad.shape {
        PadShape::Circle => ellipse_vertices(
            pad.center,
            [half_w.min(half_h), half_w.min(half_h)],
            PAD_ELLIPSE_SEGMENTS,
        ),
        PadShape::Oval => ellipse_vertices(pad.center, [half_w, half_h], PAD_ELLIPSE_SEGMENTS),
        PadShape::Rect | PadShape::RoundRect | PadShape::Trapezoid | PadShape::Custom => {
            rectangle_vertices(pad.center, pad.size_mm)
        }
    }
}

pub(super) fn pad_alpha_mul(pad_type: PadType) -> f32 {
    match pad_type {
        PadType::Connect => 0.8,
        PadType::NpThru => 0.7,
        PadType::Thru | PadType::Smd => 1.0,
    }
}

pub(super) fn with_alpha_mul(mut color: [f32; 4], alpha_mul: f32) -> [f32; 4] {
    color[3] = (color[3] * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

pub(super) fn emit_traces(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.lines.clear();
    scene.lines.reserve(snapshot.traces.len());

    let trace_color = theme.color(ColorSlot::Wire);

    for trace in &snapshot.traces {
        scene.lines.push(LineSegment {
            p0: trace.p0,
            p1: trace.p1,
            width: trace.width_mm,
            color: trace_color,
            style: trace.net,
            _pad: 0,
        });
    }
}

pub(super) fn emit_vias(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.circles.clear();
    scene.circles.reserve(snapshot.vias.len());

    let via_color = theme.color(ColorSlot::Junction);

    for via in &snapshot.vias {
        let radius = (via.diameter_mm * 0.5).max(0.01);
        let annulus = ((via.diameter_mm - via.drill_mm) * 0.5).max(0.0);

        scene.circles.push(Circle {
            center: via.center,
            radius,
            stroke_width: annulus,
            color: via_color,
        });
    }
}

pub(super) fn emit_static_polygons(
    snapshot: &PcbSnapshot,
    theme: &ResolvedTheme,
    scene: &mut Scene,
) {
    scene.polygons.clear();
    scene
        .polygons
        .reserve(snapshot.pads.len() + snapshot.zones.len() + snapshot.rule_areas.len());

    let base_fill = theme.color(ColorSlot::Pin);
    let stroke_color = theme.color(ColorSlot::SymbolBody);
    let zone_fill = with_alpha_mul(theme.color(ColorSlot::Ghost), 0.38);
    let zone_stroke = theme.color(ColorSlot::Bus);
    let rule_area_fill = with_alpha_mul(theme.color(ColorSlot::LassoFill), 0.32);
    let rule_area_stroke = theme.color(ColorSlot::LassoStroke);

    for pad in &snapshot.pads {
        let fill_color = with_alpha_mul(base_fill, pad_alpha_mul(pad.pad_type));

        let stroke_width = (pad.size_mm[0].min(pad.size_mm[1]) * 0.08).max(0.02);

        scene.polygons.push(GpuPolygon {
            vertices: pad_vertices(pad),
            fill_color,
            stroke_color: Some(stroke_color),
            stroke_width,
        });
    }

    for zone in &snapshot.zones {
        scene.polygons.push(GpuPolygon {
            vertices: zone.vertices.clone(),
            fill_color: zone_fill,
            stroke_color: Some(zone_stroke),
            stroke_width: 0.06,
        });
    }

    for rule_area in &snapshot.rule_areas {
        scene.polygons.push(GpuPolygon {
            vertices: rule_area.vertices.clone(),
            fill_color: rule_area_fill,
            stroke_color: Some(rule_area_stroke),
            stroke_width: 0.08,
        });
    }
}

pub(super) fn drc_slot(severity: Severity) -> ColorSlot {
    match severity {
        Severity::Error => ColorSlot::ErcError,
        Severity::Warning => ColorSlot::ErcWarning,
        Severity::Info => ColorSlot::ErcInfo,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DrcMarkerKind {
    Generic,
    Clearance,
    ShortCircuit,
    Unrouted,
    Dimensional,
}

pub(super) fn drc_marker_kind(marker: &DrcMarkerInput) -> DrcMarkerKind {
    match marker.violation_type {
        Some(DrcViolationType::ShortCircuit) => DrcMarkerKind::ShortCircuit,
        Some(DrcViolationType::UnroutedNet) => DrcMarkerKind::Unrouted,
        Some(
            DrcViolationType::MinTrackWidth
            | DrcViolationType::MinViaDiameter
            | DrcViolationType::MinViaDrill
            | DrcViolationType::MinHoleToHole
            | DrcViolationType::MinAnnularRing
            | DrcViolationType::MinDrill,
        ) => DrcMarkerKind::Dimensional,
        Some(
            DrcViolationType::Clearance
            | DrcViolationType::BoardOutlineClearance
            | DrcViolationType::SilkToMask
            | DrcViolationType::SilkToSilk,
        ) => DrcMarkerKind::Clearance,
        Some(
            DrcViolationType::AcuteAngle
            | DrcViolationType::AcidTrap
            | DrcViolationType::CopperSliver,
        )
        | None => DrcMarkerKind::Generic,
    }
}

pub(super) fn drc_marker_vertices(marker: &DrcMarkerInput) -> Vec<[f32; 2]> {
    let severity = marker.severity;
    let center = marker.center;
    let radius_mm = marker.radius_mm;
    let radius = radius_mm.max(0.05);
    let cx = center[0];
    let cy = center[1];

    match drc_marker_kind(marker) {
        DrcMarkerKind::ShortCircuit => vec![
            [cx, cy - radius],
            [cx + radius, cy],
            [cx, cy + radius],
            [cx - radius, cy],
        ],
        DrcMarkerKind::Unrouted => {
            ellipse_vertices(center, [radius, radius], MARKER_POLYGON_SEGMENTS)
        }
        DrcMarkerKind::Dimensional => vec![
            [cx - radius, cy - radius],
            [cx + radius, cy - radius],
            [cx + radius, cy + radius],
            [cx - radius, cy + radius],
        ],
        DrcMarkerKind::Clearance => {
            let tri_half_width = radius * 0.92;
            vec![
                [cx, cy - radius],
                [cx + tri_half_width, cy + radius * 0.5],
                [cx - tri_half_width, cy + radius * 0.5],
            ]
        }
        DrcMarkerKind::Generic => match severity {
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
            Severity::Info => ellipse_vertices(center, [radius, radius], MARKER_POLYGON_SEGMENTS),
        },
    }
}

pub(super) fn drc_marker_lines(marker: &DrcMarkerInput) -> Vec<([f32; 2], [f32; 2])> {
    let radius = marker.radius_mm.max(0.05);
    let half = radius * 0.5;
    let cx = marker.center[0];
    let cy = marker.center[1];

    match drc_marker_kind(marker) {
        DrcMarkerKind::ShortCircuit => vec![
            ([cx - half, cy - half], [cx + half, cy + half]),
            ([cx - half, cy + half], [cx + half, cy - half]),
        ],
        DrcMarkerKind::Clearance => vec![([cx, cy - half], [cx, cy + half])],
        DrcMarkerKind::Unrouted => vec![([cx - half, cy], [cx + half, cy])],
        DrcMarkerKind::Dimensional => vec![([cx - half, cy], [cx + half, cy])],
        DrcMarkerKind::Generic => match marker.severity {
            Severity::Error => vec![([cx - half, cy - half], [cx + half, cy + half])],
            Severity::Warning => vec![([cx - half, cy + half * 0.2], [cx + half, cy + half * 0.2])],
            Severity::Info => vec![([cx - half, cy], [cx + half, cy])],
        },
    }
}

pub(super) fn emit_overlays(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.overlay_lines.clear();
    scene.overlay_circles.clear();
    scene.overlay_polygons.clear();

    scene
        .overlay_lines
        .reserve(snapshot.ratsnest_lines.len() + snapshot.drc_markers.len());
    scene.overlay_circles.reserve(snapshot.drc_markers.len());
    scene.overlay_polygons.reserve(snapshot.drc_markers.len());

    let ratsnest_color = with_alpha_mul(theme.color(ColorSlot::Selection), 0.72);

    for ratsnest in &snapshot.ratsnest_lines {
        scene.overlay_lines.push(LineSegment {
            p0: ratsnest.p0,
            p1: ratsnest.p1,
            width: 0.12,
            color: ratsnest_color,
            style: RATSNEST_STYLE_DASHED | (ratsnest.net << 8),
            _pad: 0,
        });
    }

    for marker in &snapshot.drc_markers {
        let stroke_color = theme.color(drc_slot(marker.severity));
        let fill_color = with_alpha_mul(stroke_color, 0.28);

        scene.overlay_polygons.push(GpuPolygon {
            vertices: drc_marker_vertices(marker),
            fill_color,
            stroke_color: Some(stroke_color),
            stroke_width: (marker.radius_mm * 0.12).max(0.03),
        });

        scene.overlay_circles.push(Circle {
            center: marker.center,
            radius: marker.radius_mm.max(0.05),
            stroke_width: (marker.radius_mm * 0.18).max(0.04),
            color: stroke_color,
        });

        for (p0, p1) in drc_marker_lines(marker) {
            scene.overlay_lines.push(LineSegment {
                p0,
                p1,
                width: (marker.radius_mm * 0.14).max(0.04),
                color: stroke_color,
                style: 0,
                _pad: 0,
            });
        }
    }
}
