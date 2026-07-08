//! PCB 2D scene translator for the first Milestone B vertical slice.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::schematic::ViewRenderer;
use crate::theme::ResolvedTheme;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_gfx::style::ColorSlot;
use signex_types::pcb::{
    Footprint, PCB_DEFAULT_PAD_SIZE_MM, PCB_DEFAULT_TRACE_WIDTH_MM, PCB_DEFAULT_VIA_DIAMETER_MM,
    PCB_DEFAULT_VIA_DRILL_MM, PCB_TRACK_MIN_MM, PCB_VIA_MIN_DIAMETER_MM, PCB_VIA_MIN_DRILL_MM, Pad,
    PadShape, PadType, PcbBoard, Segment, Via, Zone,
};
use signex_types::schematic::Point;
use signex_types::violation::{DrcViolationType, Severity};
use std::collections::HashMap;

const PAD_ELLIPSE_SEGMENTS: usize = 18;
const MARKER_POLYGON_SEGMENTS: usize = 14;
const RATSNEST_STYLE_DASHED: u32 = 1;

#[derive(Clone, Debug)]
pub struct TraceInput {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width_mm: f32,
    pub net: u32,
}

#[derive(Clone, Debug)]
pub struct ViaInput {
    pub center: [f32; 2],
    pub diameter_mm: f32,
    pub drill_mm: f32,
    pub net: u32,
}

#[derive(Clone, Debug)]
pub struct PadInput {
    pub center: [f32; 2],
    pub size_mm: [f32; 2],
    pub shape: PadShape,
    pub pad_type: PadType,
}

#[derive(Clone, Debug)]
pub struct ZonePolygonInput {
    pub vertices: Vec<[f32; 2]>,
    pub rule_area: bool,
    pub net: u32,
    pub priority: u32,
    pub layer_rank: u16,
    pub layer_name: String,
    pub source_order: usize,
}

#[derive(Clone, Debug)]
pub struct RatsnestInput {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub net: u32,
}

#[derive(Clone, Debug)]
pub struct DrcMarkerInput {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub severity: Severity,
    pub violation_type: Option<DrcViolationType>,
}

#[derive(Clone, Debug, Default)]
pub struct PcbSnapshot {
    pub traces: Vec<TraceInput>,
    pub vias: Vec<ViaInput>,
    pub pads: Vec<PadInput>,
    pub zones: Vec<ZonePolygonInput>,
    pub rule_areas: Vec<ZonePolygonInput>,
    pub ratsnest_lines: Vec<RatsnestInput>,
    pub drc_markers: Vec<DrcMarkerInput>,
}

impl PcbSnapshot {
    pub fn from_board(board: &PcbBoard) -> Self {
        let traces = board.segments.iter().map(trace_from_segment).collect();
        let vias = board.vias.iter().map(via_from_board_via).collect();
        let layer_ranks = layer_rank_index(board);

        let mut pads = Vec::new();
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                pads.push(pad_from_footprint(footprint, pad));
            }
        }

        let mut zones = Vec::new();
        let mut rule_areas = Vec::new();

        for (source_order, zone) in board.zones.iter().enumerate() {
            if let Some(zone_polygon) = zone_from_board_zone(zone, source_order, &layer_ranks) {
                if zone_polygon.rule_area {
                    rule_areas.push(zone_polygon);
                } else {
                    zones.push(zone_polygon);
                }
            }
        }

        sort_zone_stack(&mut zones);
        sort_zone_stack(&mut rule_areas);

        Self {
            traces,
            vias,
            pads,
            zones,
            rule_areas,
            ratsnest_lines: Vec::new(),
            drc_markers: Vec::new(),
        }
    }

    pub fn with_ratsnest_lines(mut self, ratsnest_lines: Vec<RatsnestInput>) -> Self {
        self.ratsnest_lines = ratsnest_lines;
        self
    }

    pub fn with_drc_markers(mut self, drc_markers: Vec<DrcMarkerInput>) -> Self {
        self.drc_markers = drc_markers;
        self
    }
}

fn trace_from_segment(segment: &Segment) -> TraceInput {
    TraceInput {
        p0: point_to_xy(segment.start),
        p1: point_to_xy(segment.end),
        width_mm: mm_with_floor(segment.width, PCB_DEFAULT_TRACE_WIDTH_MM, PCB_TRACK_MIN_MM),
        net: segment.net,
    }
}

fn via_from_board_via(via: &Via) -> ViaInput {
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

fn pad_from_footprint(footprint: &Footprint, pad: &Pad) -> PadInput {
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

fn zone_from_board_zone(
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

fn layer_rank_index(board: &PcbBoard) -> HashMap<String, u16> {
    let mut index = HashMap::with_capacity(board.layers.len());

    for layer in &board.layers {
        index.insert(layer.name.to_ascii_lowercase(), u16::from(layer.id));
    }

    index
}

fn sort_zone_stack(zones: &mut [ZonePolygonInput]) {
    zones.sort_by(|a, b| {
        a.layer_rank
            .cmp(&b.layer_rank)
            .then_with(|| a.layer_name.cmp(&b.layer_name))
            .then_with(|| zone_layer_top_composite_key(a).cmp(&zone_layer_top_composite_key(b)))
            .then_with(|| a.source_order.cmp(&b.source_order))
    });
}

fn zone_layer_top_composite_key(zone: &ZonePolygonInput) -> (u32, u8, u32) {
    // Ascending order is intentional: lower priority and net bucket are painted first,
    // so higher-priority connected zones land on top in the same layer stack.
    (zone.priority, zone_connected_bucket(zone.net), zone.net)
}

fn zone_connected_bucket(net: u32) -> u8 {
    if net == 0 { 0 } else { 1 }
}

fn is_rule_area_zone(zone: &Zone) -> bool {
    let fill = zone.fill_type.to_ascii_lowercase();
    let layer = zone.layer.to_ascii_lowercase();

    fill.contains("rule") || fill.contains("keepout") || layer.contains("rule")
}

fn point_to_xy(point: Point) -> [f32; 2] {
    [point.x as f32, point.y as f32]
}

fn mm_with_floor(value: f64, fallback: f64, floor: f64) -> f32 {
    let candidate = if value > 0.0 { value } else { fallback };
    candidate.max(floor) as f32
}

fn rotate_local(point: [f32; 2], rotation_deg: f32) -> [f32; 2] {
    let angle = rotation_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    [
        point[0] * cos_a - point[1] * sin_a,
        point[0] * sin_a + point[1] * cos_a,
    ]
}

fn rectangle_vertices(center: [f32; 2], size_mm: [f32; 2]) -> Vec<[f32; 2]> {
    let half_w = (size_mm[0] * 0.5).max(0.01);
    let half_h = (size_mm[1] * 0.5).max(0.01);

    vec![
        [center[0] - half_w, center[1] - half_h],
        [center[0] + half_w, center[1] - half_h],
        [center[0] + half_w, center[1] + half_h],
        [center[0] - half_w, center[1] + half_h],
    ]
}

fn ellipse_vertices(center: [f32; 2], radius_xy: [f32; 2], segments: usize) -> Vec<[f32; 2]> {
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

fn pad_vertices(pad: &PadInput) -> Vec<[f32; 2]> {
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

fn pad_alpha_mul(pad_type: PadType) -> f32 {
    match pad_type {
        PadType::Connect => 0.8,
        PadType::NpThru => 0.7,
        PadType::Thru | PadType::Smd => 1.0,
    }
}

fn with_alpha_mul(mut color: [f32; 4], alpha_mul: f32) -> [f32; 4] {
    color[3] = (color[3] * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

fn emit_traces(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
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

fn emit_vias(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
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

fn emit_static_polygons(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
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

fn drc_slot(severity: Severity) -> ColorSlot {
    match severity {
        Severity::Error => ColorSlot::ErcError,
        Severity::Warning => ColorSlot::ErcWarning,
        Severity::Info => ColorSlot::ErcInfo,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrcMarkerKind {
    Generic,
    Clearance,
    ShortCircuit,
    Unrouted,
    Dimensional,
}

fn drc_marker_kind(marker: &DrcMarkerInput) -> DrcMarkerKind {
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

fn drc_marker_vertices(marker: &DrcMarkerInput) -> Vec<[f32; 2]> {
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

fn drc_marker_lines(marker: &DrcMarkerInput) -> Vec<([f32; 2], [f32; 2])> {
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

fn emit_overlays(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcbSliceFamily {
    Traces,
    Vias,
    Pads,
    Zones,
    RuleAreas,
    Ratsnest,
    Drc,
    Theme,
    Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcbAppEvent {
    TraceEdited,
    ViaEdited,
    PadEdited,
    FootprintMoved,
    ZoneRefilled,
    RuleAreaUpdated,
    RatsnestRebuilt,
    DrcResultsUpdated,
    ThemeChanged,
    CameraMoved,
}

const FAMILIES_TRACE_EDITED: &[PcbSliceFamily] =
    &[PcbSliceFamily::Traces, PcbSliceFamily::Ratsnest];
const FAMILIES_VIA_EDITED: &[PcbSliceFamily] = &[PcbSliceFamily::Vias, PcbSliceFamily::Traces];
const FAMILIES_PAD_EDITED: &[PcbSliceFamily] = &[PcbSliceFamily::Pads, PcbSliceFamily::Traces];
const FAMILIES_FOOTPRINT_MOVED: &[PcbSliceFamily] = &[PcbSliceFamily::Pads, PcbSliceFamily::Traces];
const FAMILIES_ZONE_REFILLED: &[PcbSliceFamily] = &[PcbSliceFamily::Zones];
const FAMILIES_RULE_AREA_UPDATED: &[PcbSliceFamily] =
    &[PcbSliceFamily::RuleAreas, PcbSliceFamily::Drc];
const FAMILIES_RATSNEST_REBUILT: &[PcbSliceFamily] = &[PcbSliceFamily::Ratsnest];
const FAMILIES_DRC_RESULTS_UPDATED: &[PcbSliceFamily] = &[PcbSliceFamily::Drc];
const FAMILIES_THEME_CHANGED: &[PcbSliceFamily] = &[PcbSliceFamily::Theme];
const FAMILIES_CAMERA_MOVED: &[PcbSliceFamily] = &[PcbSliceFamily::Camera];

pub fn families_for_event(event: PcbAppEvent) -> &'static [PcbSliceFamily] {
    match event {
        PcbAppEvent::TraceEdited => FAMILIES_TRACE_EDITED,
        PcbAppEvent::ViaEdited => FAMILIES_VIA_EDITED,
        PcbAppEvent::PadEdited => FAMILIES_PAD_EDITED,
        PcbAppEvent::FootprintMoved => FAMILIES_FOOTPRINT_MOVED,
        PcbAppEvent::ZoneRefilled => FAMILIES_ZONE_REFILLED,
        PcbAppEvent::RuleAreaUpdated => FAMILIES_RULE_AREA_UPDATED,
        PcbAppEvent::RatsnestRebuilt => FAMILIES_RATSNEST_REBUILT,
        PcbAppEvent::DrcResultsUpdated => FAMILIES_DRC_RESULTS_UPDATED,
        PcbAppEvent::ThemeChanged => FAMILIES_THEME_CHANGED,
        PcbAppEvent::CameraMoved => FAMILIES_CAMERA_MOVED,
    }
}

pub fn dirty_flags_for_families(families: &[PcbSliceFamily]) -> DirtyFlags {
    let mut dirty = DirtyFlags::empty();

    for family in families {
        dirty |= match family {
            PcbSliceFamily::Traces => DirtyFlags::LINES,
            PcbSliceFamily::Vias => DirtyFlags::CIRCLES,
            PcbSliceFamily::Pads | PcbSliceFamily::Zones | PcbSliceFamily::RuleAreas => {
                DirtyFlags::POLYGONS
            }
            PcbSliceFamily::Ratsnest | PcbSliceFamily::Drc => DirtyFlags::OVERLAY,
            PcbSliceFamily::Theme => DirtyFlags::THEME,
            PcbSliceFamily::Camera => DirtyFlags::empty(),
        };
    }

    dirty
}

pub fn dirty_flags_for_event(event: PcbAppEvent) -> DirtyFlags {
    dirty_flags_for_families(families_for_event(event))
}

pub fn dirty_flags_for_events(events: &[PcbAppEvent]) -> DirtyFlags {
    let mut dirty = DirtyFlags::empty();

    for event in events {
        dirty |= dirty_flags_for_event(*event);
    }

    dirty
}

pub struct PcbRenderer;

impl ViewRenderer for PcbRenderer {
    type Snapshot = PcbSnapshot;

    fn build_scene(
        snapshot: &Self::Snapshot,
        theme: &ResolvedTheme,
        dirty: DirtyFlags,
        scene: &mut Scene,
    ) {
        if dirty.contains(DirtyFlags::LINES) {
            emit_traces(snapshot, theme, scene);
        }

        if dirty.contains(DirtyFlags::CIRCLES) {
            emit_vias(snapshot, theme, scene);
        }

        if dirty.contains(DirtyFlags::POLYGONS) {
            emit_static_polygons(snapshot, theme, scene);
        }

        if dirty.contains(DirtyFlags::OVERLAY) {
            emit_overlays(snapshot, theme, scene);
        }

        scene.dirty |= dirty;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DrcMarkerInput, PcbAppEvent, PcbRenderer, PcbSliceFamily, PcbSnapshot, RatsnestInput,
        ZonePolygonInput, dirty_flags_for_event, dirty_flags_for_events, dirty_flags_for_families,
        sort_zone_stack,
    };
    use crate::schematic::ViewRenderer;
    use crate::theme::ResolvedTheme;
    use signex_gfx::scene::{DirtyFlags, Scene};
    use signex_types::pcb::PcbBoard;
    use signex_types::violation::{DrcViolationType, Severity};

    fn sample_board() -> PcbBoard {
        serde_json::from_str(
            r#"
            {
              "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651",
              "version": 1,
              "generator": "test",
              "thickness": 1.6,
              "outline": [],
                            "layers": [
                                { "id": 0, "name": "F.Cu", "layer_type": "signal" },
                                { "id": 31, "name": "B.Cu", "layer_type": "signal" }
                            ],
              "setup": null,
              "nets": [
                { "number": 1, "name": "VCC" },
                { "number": 2, "name": "GND" }
              ],
              "footprints": [
                {
                  "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
                  "reference": "U1",
                  "value": "MCU",
                  "footprint_id": "QFN-16",
                  "position": { "x": 10.0, "y": 20.0 },
                  "rotation": 90.0,
                  "layer": "F.Cu",
                  "locked": false,
                  "pads": [
                    {
                      "uuid": "47ceef20-d496-4e62-b8d8-7c34195c6818",
                      "number": "1",
                      "pad_type": "smd",
                      "shape": "rect",
                      "position": { "x": 2.0, "y": 0.0 },
                      "size": { "x": 1.5, "y": 0.8 },
                      "drill": null,
                      "layers": ["F.Cu"],
                      "net": { "number": 1, "name": "VCC" },
                      "roundrect_ratio": 0.0
                    }
                  ],
                  "graphics": [],
                  "properties": []
                }
              ],
              "segments": [
                {
                  "uuid": "f5ea9948-4c2f-4501-b647-4047f6eefca9",
                  "start": { "x": 0.0, "y": 0.0 },
                  "end": { "x": 5.0, "y": 0.0 },
                  "width": 0.25,
                  "layer": "F.Cu",
                  "net": 1
                }
              ],
              "vias": [
                {
                  "uuid": "7f2eb13d-a3d6-47e6-84f7-3b27240a95f3",
                  "position": { "x": 2.5, "y": 1.0 },
                  "diameter": 0.8,
                  "drill": 0.4,
                  "layers": ["F.Cu", "B.Cu"],
                  "net": 2
                }
              ],
                            "zones": [
                                {
                                    "uuid": "d26f3069-2d89-4f44-ab9a-24a583fde0d7",
                                    "net": 1,
                                    "net_name": "VCC",
                                    "layer": "F.Cu",
                                    "outline": [
                                        { "x": 0.0, "y": 0.0 },
                                        { "x": 4.0, "y": 0.0 },
                                        { "x": 4.0, "y": 2.0 },
                                        { "x": 0.0, "y": 2.0 }
                                    ],
                                    "priority": 7,
                                    "fill_type": "solid"
                                },
                                {
                                    "uuid": "4cec6cfd-a5d8-450d-bf31-c8f2fd8f5b39",
                                    "net": 2,
                                    "net_name": "GND",
                                    "layer": "F.Cu",
                                    "outline": [
                                        { "x": 0.5, "y": 0.5 },
                                        { "x": 2.0, "y": 0.5 },
                                        { "x": 2.0, "y": 1.5 },
                                        { "x": 0.5, "y": 1.5 }
                                    ],
                                    "priority": 2,
                                    "fill_type": "solid"
                                },
                                {
                                    "uuid": "9d40ec94-a20d-4a89-9a6c-a3124ccf4c40",
                                    "net": 0,
                                    "net_name": "",
                                    "layer": "Cmts.User",
                                    "outline": [
                                        { "x": 6.0, "y": 1.0 },
                                        { "x": 9.0, "y": 1.0 },
                                        { "x": 9.0, "y": 3.0 },
                                        { "x": 6.0, "y": 3.0 }
                                    ],
                                    "priority": 2,
                                    "fill_type": "rule_area"
                                }
                            ],
              "graphics": [],
              "texts": []
            }
            "#,
        )
        .expect("valid sample pcb board")
    }

    #[test]
    fn pcb_snapshot_collects_trace_via_pad_and_zone_inputs() {
        let board = sample_board();
        let snapshot = PcbSnapshot::from_board(&board);

        assert_eq!(snapshot.traces.len(), 1);
        assert_eq!(snapshot.vias.len(), 1);
        assert_eq!(snapshot.pads.len(), 1);
        assert_eq!(snapshot.zones.len(), 2);
        assert_eq!(snapshot.rule_areas.len(), 1);

        assert_eq!(snapshot.traces[0].width_mm, 0.25);
        assert_eq!(snapshot.vias[0].diameter_mm, 0.8);
        assert_eq!(snapshot.pads[0].center[0], 10.0);
        assert_eq!(snapshot.pads[0].center[1], 22.0);
        assert_eq!(snapshot.zones[0].vertices.len(), 4);
        assert_eq!(snapshot.rule_areas[0].vertices.len(), 4);
        assert_eq!(snapshot.zones[0].priority, 2);
        assert_eq!(snapshot.zones[1].priority, 7);
        assert!(snapshot.rule_areas[0].rule_area);
    }

    #[test]
    fn pcb_renderer_updates_each_family_with_matching_dirty_flag() {
        let board = sample_board();
        let snapshot = PcbSnapshot::from_board(&board);
        let theme = ResolvedTheme::builtin_default();
        let mut scene = Scene::default();

        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 0);
        assert_eq!(scene.polygons.len(), 0);

        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::CIRCLES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.polygons.len(), 0);

        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::POLYGONS, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.polygons.len(), 4);
        assert_eq!(scene.lines[0].style, 1);
    }

    #[test]
    fn pcb_overlay_slice_emits_ratsnest_and_drc_primitives() {
        let board = sample_board();
        let snapshot = PcbSnapshot::from_board(&board)
            .with_ratsnest_lines(vec![RatsnestInput {
                p0: [0.0, 0.0],
                p1: [3.0, 2.0],
                net: 1,
            }])
            .with_drc_markers(vec![DrcMarkerInput {
                center: [2.0, 2.0],
                radius_mm: 0.35,
                severity: Severity::Error,
                violation_type: None,
            }]);

        let theme = ResolvedTheme::builtin_default();
        let mut scene = Scene::default();

        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);

        assert_eq!(scene.overlay_lines.len(), 2);
        assert_eq!(scene.overlay_circles.len(), 1);
        assert_eq!(scene.overlay_polygons.len(), 1);
        assert_eq!(scene.overlay_lines[0].style & 1, 1);
    }

    #[test]
    fn pcb_drc_violation_type_expands_overlay_line_patterns() {
        let board = sample_board();
        let snapshot = PcbSnapshot::from_board(&board).with_drc_markers(vec![
            DrcMarkerInput {
                center: [2.0, 2.0],
                radius_mm: 0.35,
                severity: Severity::Error,
                violation_type: Some(DrcViolationType::ShortCircuit),
            },
            DrcMarkerInput {
                center: [3.5, 2.0],
                radius_mm: 0.3,
                severity: Severity::Warning,
                violation_type: Some(DrcViolationType::MinTrackWidth),
            },
        ]);

        let theme = ResolvedTheme::builtin_default();
        let mut scene = Scene::default();
        PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);

        assert_eq!(scene.overlay_polygons.len(), 2);
        assert_eq!(scene.overlay_circles.len(), 2);
        assert_eq!(scene.overlay_lines.len(), 3);
    }

    #[test]
    fn pcb_zone_sort_prefers_priority_then_connected_net_for_layer_top() {
        let mut zones = vec![
            ZonePolygonInput {
                vertices: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                rule_area: false,
                net: 3,
                priority: 2,
                layer_rank: 0,
                layer_name: "f.cu".to_string(),
                source_order: 2,
            },
            ZonePolygonInput {
                vertices: vec![[2.0, 0.0], [3.0, 0.0], [3.0, 1.0]],
                rule_area: false,
                net: 0,
                priority: 2,
                layer_rank: 0,
                layer_name: "f.cu".to_string(),
                source_order: 1,
            },
            ZonePolygonInput {
                vertices: vec![[4.0, 0.0], [5.0, 0.0], [5.0, 1.0]],
                rule_area: false,
                net: 5,
                priority: 1,
                layer_rank: 0,
                layer_name: "f.cu".to_string(),
                source_order: 0,
            },
        ];

        sort_zone_stack(&mut zones);

        let ordered: Vec<(u32, u32)> = zones.iter().map(|zone| (zone.priority, zone.net)).collect();
        assert_eq!(ordered, vec![(1, 5), (2, 0), (2, 3)]);
    }

    #[test]
    fn pcb_slice_dirty_mapping_resolves_expected_flags() {
        let dirty = dirty_flags_for_families(&[
            PcbSliceFamily::Traces,
            PcbSliceFamily::Zones,
            PcbSliceFamily::Ratsnest,
            PcbSliceFamily::Theme,
        ]);

        assert!(dirty.contains(DirtyFlags::LINES));
        assert!(dirty.contains(DirtyFlags::POLYGONS));
        assert!(dirty.contains(DirtyFlags::OVERLAY));
        assert!(dirty.contains(DirtyFlags::THEME));
        assert!(!dirty.contains(DirtyFlags::CIRCLES));
    }

    #[test]
    fn pcb_event_mapping_routes_to_expected_dirty_flags() {
        let trace_dirty = dirty_flags_for_event(PcbAppEvent::TraceEdited);
        assert!(trace_dirty.contains(DirtyFlags::LINES));
        assert!(trace_dirty.contains(DirtyFlags::OVERLAY));

        let zone_dirty = dirty_flags_for_event(PcbAppEvent::ZoneRefilled);
        assert!(zone_dirty.contains(DirtyFlags::POLYGONS));
        assert!(!zone_dirty.contains(DirtyFlags::LINES));

        let camera_dirty = dirty_flags_for_event(PcbAppEvent::CameraMoved);
        assert_eq!(camera_dirty, DirtyFlags::empty());

        let combined = dirty_flags_for_events(&[
            PcbAppEvent::ViaEdited,
            PcbAppEvent::DrcResultsUpdated,
            PcbAppEvent::ThemeChanged,
        ]);

        assert!(combined.contains(DirtyFlags::LINES));
        assert!(combined.contains(DirtyFlags::CIRCLES));
        assert!(combined.contains(DirtyFlags::OVERLAY));
        assert!(combined.contains(DirtyFlags::THEME));
    }
}
