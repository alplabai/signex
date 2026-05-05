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
    Footprint, Pad, PadShape, PadType, PcbBoard, Segment, Via, PCB_DEFAULT_PAD_SIZE_MM,
    PCB_DEFAULT_TRACE_WIDTH_MM, PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM,
    PCB_TRACK_MIN_MM, PCB_VIA_MIN_DIAMETER_MM, PCB_VIA_MIN_DRILL_MM,
};
use signex_types::schematic::Point;

const PAD_ELLIPSE_SEGMENTS: usize = 18;

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

#[derive(Clone, Debug, Default)]
pub struct PcbSnapshot {
    pub traces: Vec<TraceInput>,
    pub vias: Vec<ViaInput>,
    pub pads: Vec<PadInput>,
}

impl PcbSnapshot {
    pub fn from_board(board: &PcbBoard) -> Self {
        let traces = board.segments.iter().map(trace_from_segment).collect();
        let vias = board.vias.iter().map(via_from_board_via).collect();

        let mut pads = Vec::new();
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                pads.push(pad_from_footprint(footprint, pad));
            }
        }

        Self { traces, vias, pads }
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
        diameter_mm: mm_with_floor(via.diameter, PCB_DEFAULT_VIA_DIAMETER_MM, PCB_VIA_MIN_DIAMETER_MM),
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
        PadShape::Rect
        | PadShape::RoundRect
        | PadShape::Trapezoid
        | PadShape::Custom => rectangle_vertices(pad.center, pad.size_mm),
    }
}

fn pad_alpha_mul(pad_type: PadType) -> f32 {
    match pad_type {
        PadType::Connect => 0.8,
        PadType::NpThru => 0.7,
        PadType::Thru | PadType::Smd => 1.0,
    }
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

fn emit_pads(snapshot: &PcbSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.polygons.clear();
    scene.polygons.reserve(snapshot.pads.len());

    let base_fill = theme.color(ColorSlot::Pin);
    let stroke_color = theme.color(ColorSlot::SymbolBody);

    for pad in &snapshot.pads {
        let mut fill_color = base_fill;
        fill_color[3] = (fill_color[3] * pad_alpha_mul(pad.pad_type)).clamp(0.0, 1.0);

        let stroke_width = (pad.size_mm[0].min(pad.size_mm[1]) * 0.08).max(0.02);

        scene.polygons.push(GpuPolygon {
            vertices: pad_vertices(pad),
            fill_color,
            stroke_color: Some(stroke_color),
            stroke_width,
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcbSliceFamily {
    Traces,
    Vias,
    Pads,
    Theme,
}

pub fn dirty_flags_for_families(families: &[PcbSliceFamily]) -> DirtyFlags {
    let mut dirty = DirtyFlags::empty();

    for family in families {
        dirty |= match family {
            PcbSliceFamily::Traces => DirtyFlags::LINES,
            PcbSliceFamily::Vias => DirtyFlags::CIRCLES,
            PcbSliceFamily::Pads => DirtyFlags::POLYGONS,
            PcbSliceFamily::Theme => DirtyFlags::THEME,
        };
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
            emit_pads(snapshot, theme, scene);
        }

        scene.dirty |= dirty;
    }
}

#[cfg(test)]
mod tests {
    use super::{dirty_flags_for_families, PcbRenderer, PcbSliceFamily, PcbSnapshot};
    use crate::schematic::ViewRenderer;
    use crate::theme::ResolvedTheme;
    use signex_gfx::scene::{DirtyFlags, Scene};
    use signex_types::pcb::PcbBoard;

    fn sample_board() -> PcbBoard {
        serde_json::from_str(
            r#"
            {
              "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651",
              "version": 1,
              "generator": "test",
              "thickness": 1.6,
              "outline": [],
              "layers": [],
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
              "zones": [],
              "graphics": [],
              "texts": []
            }
            "#,
        )
        .expect("valid sample pcb board")
    }

    #[test]
    fn pcb_snapshot_collects_trace_via_and_pad_inputs() {
        let board = sample_board();
        let snapshot = PcbSnapshot::from_board(&board);

        assert_eq!(snapshot.traces.len(), 1);
        assert_eq!(snapshot.vias.len(), 1);
        assert_eq!(snapshot.pads.len(), 1);

        assert_eq!(snapshot.traces[0].width_mm, 0.25);
        assert_eq!(snapshot.vias[0].diameter_mm, 0.8);
        assert_eq!(snapshot.pads[0].center[0], 10.0);
        assert_eq!(snapshot.pads[0].center[1], 22.0);
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
        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.lines[0].style, 1);
    }

    #[test]
    fn pcb_slice_dirty_mapping_resolves_expected_flags() {
        let dirty = dirty_flags_for_families(&[
            PcbSliceFamily::Traces,
            PcbSliceFamily::Pads,
            PcbSliceFamily::Theme,
        ]);

        assert!(dirty.contains(DirtyFlags::LINES));
        assert!(dirty.contains(DirtyFlags::POLYGONS));
        assert!(dirty.contains(DirtyFlags::THEME));
        assert!(!dirty.contains(DirtyFlags::CIRCLES));
    }
}
