//! PCB scene emitters + snapshot geometry helpers.

use super::*;

pub(super) fn trace_from_segment(segment: &Segment) -> TraceInput {
    TraceInput {
        p0: point_to_xy(segment.start),
        p1: point_to_xy(segment.end),
        width_mm: mm_with_floor(segment.width, PCB_DEFAULT_TRACE_WIDTH_MM, PCB_TRACK_MIN_MM),
        net: segment.net,
        dimmed: false,
        ink: CopperInk::Unknown,
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
        dimmed: false,
        ink: CopperInk::Unknown,
    }
}

/// A pad as drawn. The outline is `pcb-geom`'s own pad shape (shape,
/// parameters, footprint turn and the pad's own turn composed; a bottom-side
/// footprint is mirrored there, ADR-0005 §7), so a hit-test and a ratsnest end
/// land on what is drawn. A pad `pcb-geom` refuses (non-finite or beyond
/// 1 km) is not drawn; `BoardIndex::rejected` reports it.
pub(super) fn pad_from_footprint(footprint: &Footprint, pad: &Pad) -> Option<PadInput> {
    let center = pcb_geom::pad_world_center(footprint, pad).ok()?.to_mm();

    // Size floor is a renderer rule, not geometry (ADR-0005, "Drawn size").
    let size_mm = [
        mm_with_floor(pad.size.x, PCB_DEFAULT_PAD_SIZE_MM, 0.05),
        mm_with_floor(pad.size.y, PCB_DEFAULT_PAD_SIZE_MM, 0.05),
    ];
    let mut drawn = pad.clone();
    drawn.size = signex_types::pcb::Point::new(f64::from(size_mm[0]), f64::from(size_mm[1]));
    let outline = pcb_geom::pad_shape(footprint, &drawn)
        .ok()?
        .ring(PAD_ARC_STEPS);
    Some(PadInput {
        center: point_to_xy(center),
        size_mm,
        outline: outline
            .into_iter()
            .map(|p| point_to_xy(p.to_mm()))
            .collect(),
        pad_type: pad.pad_type,
        drill_mm: pad_drill_mm(pad),
        dimmed: false,
        ink: CopperInk::Unknown,
    })
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
    // Only holes that keep the `Zone.holes` invariant reach either render
    // path, so the GPU fill and the CPU even-odd fill draw the same zone;
    // the app reports the rest when the board is opened (#897).
    let holes = zone
        .usable_holes()
        .into_iter()
        .map(|i| {
            zone.holes[i]
                .iter()
                .map(|point| point_to_xy(*point))
                .collect()
        })
        .collect();
    let layer_name = zone.layer.to_ascii_lowercase();
    let layer_rank = layer_ranks.get(&layer_name).copied().unwrap_or(u16::MAX);

    Some(ZonePolygonInput {
        vertices,
        holes,
        rule_area: is_rule_area_zone(zone),
        net: zone.net,
        priority: zone.priority,
        layer_rank,
        layer_name,
        source_order,
        dimmed: false,
        ink: CopperInk::Unknown,
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

pub(super) fn pad_alpha_mul(pad_type: PadType) -> f32 {
    match pad_type {
        PadType::Connect => 0.8,
        PadType::NpThru => 0.7,
        PadType::Thru | PadType::Smd => 1.0,
    }
}

/// `color` faint when `dimmed`: copper off the active layer.
pub(super) fn dim_if(color: [f32; 4], dimmed: bool) -> [f32; 4] {
    if dimmed {
        with_alpha_mul(color, DIMMED_ALPHA)
    } else {
        color
    }
}

pub(super) fn with_alpha_mul(mut color: [f32; 4], alpha_mul: f32) -> [f32; 4] {
    color[3] = (color[3] * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

/// The diameter of a through-hole pad's drill, 0 when it has none.
fn pad_drill_mm(pad: &Pad) -> f32 {
    let through = matches!(pad.pad_type, PadType::Thru | PadType::NpThru);
    match &pad.drill {
        Some(drill) if through && drill.diameter.is_finite() && drill.diameter > 0.0 => {
            drill.diameter as f32
        }
        _ => 0.0,
    }
}

/// The colour of copper drawn in `ink`; `fallback` for copper on no layer
/// this build knows.
pub(super) fn copper_color(theme: &ResolvedTheme, ink: CopperInk, fallback: ColorSlot) -> [f32; 4] {
    theme.color(ink.slot(fallback))
}

pub(super) fn emit_traces(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.lines.clear();
    scene.lines.reserve(snapshot.traces.len());

    for trace in &snapshot.traces {
        let color = copper_color(theme, trace.ink, ColorSlot::Wire);
        scene.lines.push(LineSegment {
            p0: trace.p0,
            p1: trace.p1,
            width: trace.width_mm,
            color: dim_if(color, trace.dimmed),
            // Bit 0 of `style` is the dash flag. A trace's net, if it is ever
            // encoded here, goes above bit 8 like the ratsnest's.
            style: 0,
            _pad: 0,
        });
    }
}

pub(super) fn emit_vias(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.circles.clear();
    scene.circles.reserve(snapshot.vias.len() * 2);

    let opening = theme.color(ColorSlot::BoardOpening);

    for via in &snapshot.vias {
        let outer = (via.diameter_mm * 0.5).max(0.01);
        let hole = (via.drill_mm * 0.5).clamp(0.0, outer);
        let color = copper_color(theme, via.ink, ColorSlot::Junction);

        // Both renderers centre a ring's stroke on its radius, so stroke the
        // annulus at the midpoint between the hole edge and the outer edge.
        scene.circles.push(Circle {
            center: via.center,
            radius: (outer + hole) * 0.5,
            stroke_width: outer - hole,
            color: dim_if(color, via.dimmed),
        });
        // The drill shows as an opening, not the trace under it. A drill as
        // wide as the via leaves no annulus and draws as a copper disc.
        if hole > 0.0 && hole < outer {
            scene.circles.push(Circle {
                center: via.center,
                radius: hole,
                stroke_width: 0.0,
                color: opening,
            });
        }
    }
}

pub(super) fn emit_static_polygons(
    snapshot: &PcbSnapshot,
    theme: &ResolvedTheme,
    scene: &mut Scene,
) {
    // Zones and rule areas stay in `polygons` (under the traces); pads go to
    // `top_polygons`, which composites above traces and vias.
    scene.polygons.clear();
    scene
        .polygons
        .reserve(snapshot.zones.len() + snapshot.rule_areas.len());
    scene.top_polygons.clear();
    scene.top_polygons.reserve(snapshot.pads.len());

    // The board area first, under every zone and all copper. Cutouts and
    // mounting holes stay open: the overlay draws them over it.
    let outline = &snapshot.board_features.outline;
    if outline.len() >= 3 {
        scene.polygons.push(GpuPolygon {
            vertices: outline.clone(),
            holes: Vec::new(),
            fill_color: theme.color(ColorSlot::BoardArea),
            stroke_color: None,
            stroke_width: 0.0,
        });
    }

    let opening = theme.color(ColorSlot::BoardOpening);
    let rule_area_fill = with_alpha_mul(theme.color(ColorSlot::LassoFill), 0.32);
    let rule_area_stroke = theme.color(ColorSlot::LassoStroke);

    // A pad is its copper outline alone: `pcb-geom` gives the exact shape,
    // and an outline stroke would widen it.
    for pad in &snapshot.pads {
        let copper = copper_color(theme, pad.ink, ColorSlot::Pin);
        let fill_color = dim_if(
            with_alpha_mul(copper, pad_alpha_mul(pad.pad_type)),
            pad.dimmed,
        );
        scene.top_polygons.push(GpuPolygon {
            holes: Vec::new(),
            vertices: pad.outline.clone(),
            fill_color,
            stroke_color: None,
            stroke_width: 0.0,
        });
        // The drill right after its pad, so it draws over the copper.
        if pad.drill_mm > 0.0 {
            let radius = pad.drill_mm * 0.5;
            scene.top_polygons.push(GpuPolygon {
                holes: Vec::new(),
                vertices: ellipse_vertices(pad.center, [radius, radius], DRILL_SEGMENTS),
                fill_color: opening,
                stroke_color: None,
                stroke_width: 0.0,
            });
        }
    }

    for zone in &snapshot.zones {
        let fill = with_alpha_mul(copper_color(theme, zone.ink, ColorSlot::Ghost), 0.38);
        let stroke = copper_color(theme, zone.ink, ColorSlot::Bus);
        scene.polygons.push(GpuPolygon {
            vertices: zone.vertices.clone(),
            holes: zone.holes.clone(),
            fill_color: dim_if(fill, zone.dimmed),
            stroke_color: Some(dim_if(stroke, zone.dimmed)),
            stroke_width: 0.06,
        });
    }

    for rule_area in &snapshot.rule_areas {
        scene.polygons.push(GpuPolygon {
            vertices: rule_area.vertices.clone(),
            holes: rule_area.holes.clone(),
            fill_color: rule_area_fill,
            stroke_color: Some(rule_area_stroke),
            stroke_width: 0.08,
        });
    }
}

pub(super) fn drc_slot(severity: Severity) -> ColorSlot {
    match severity {
        Severity::Error => ColorSlot::DrcError,
        Severity::Warning => ColorSlot::DrcWarning,
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

    // The board's openings and keepouts first, so the ratsnest and DRC
    // markers stay readable on top (#2234).
    super::board_features::emit(&snapshot.board_features, theme, scene);

    let ratsnest_color = with_alpha_mul(theme.color(ColorSlot::Ratsnest), 0.72);

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
            holes: Vec::new(),
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
