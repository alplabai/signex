//! Schematic renderer interfaces.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::theme::ResolvedTheme;
use signex_gfx::scene::{DirtyFlags, Scene};
use std::collections::HashMap;

use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::style::{ColorSlot, StyleRef};
use signex_types::schematic::{HAlign, VAlign};
use signex_types::violation::Severity;

/// Common view renderer contract used by scene translators.
pub trait ViewRenderer {
    type Snapshot;

    fn build_scene(
        snapshot: &Self::Snapshot,
        theme: &ResolvedTheme,
        dirty: DirtyFlags,
        scene: &mut Scene,
    );
}

#[derive(Clone, Debug)]
pub struct WireInput {
    pub id: u64,
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width_mm: f32,
    pub explicit_color: Option<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct JunctionInput {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct ArcInput {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub start_angle_rad: f32,
    pub end_angle_rad: f32,
    pub width_mm: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct PolygonInput {
    pub vertices: Vec<[f32; 2]>,
    pub fill_color: [f32; 4],
    pub stroke_color: Option<[f32; 4]>,
    pub stroke_width_mm: f32,
}

#[derive(Clone, Debug)]
pub struct TextInput {
    pub content: String,
    pub position: [f32; 2],
    pub size_mm: f32,
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub rotation_rad: f32,
    pub h_align: HAlign,
    pub v_align: VAlign,
}

#[derive(Clone, Debug)]
pub struct OverlayLineInput {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width_mm: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct OverlayCircleInput {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub stroke_width_mm: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct OverlayPolygonInput {
    pub vertices: Vec<[f32; 2]>,
    pub fill_color: [f32; 4],
    pub stroke_color: Option<[f32; 4]>,
    pub stroke_width_mm: f32,
}

#[derive(Clone, Debug, Default)]
pub struct OverlayInputs {
    pub preview_lines: Vec<OverlayLineInput>,
    pub ghost_polygons: Vec<OverlayPolygonInput>,
    pub lasso_lines: Vec<OverlayLineInput>,
    pub snap_circles: Vec<OverlayCircleInput>,
}

#[derive(Clone, Debug)]
pub struct ErcMarkerInput {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub severity: Severity,
}

#[derive(Clone, Debug)]
pub struct SchematicSnapshot {
    pub wires: Vec<WireInput>,
    pub junctions: Vec<JunctionInput>,
    pub arcs: Vec<ArcInput>,
    pub polygons: Vec<PolygonInput>,
    pub labels: Vec<TextInput>,
    pub pin_texts: Vec<TextInput>,
    pub reference_value_texts: Vec<TextInput>,
    pub parameter_texts: Vec<TextInput>,
    pub overlays: OverlayInputs,
    pub erc_markers: Vec<ErcMarkerInput>,
    pub wire_color_overrides: HashMap<u64, [f32; 4]>,
}

fn resolve_wire_color(wire: &WireInput, snapshot: &SchematicSnapshot, theme: &ResolvedTheme) -> [f32; 4] {
    snapshot
        .wire_color_overrides
        .get(&wire.id)
        .copied()
        .or(wire.explicit_color)
        .unwrap_or(theme.color(ColorSlot::Wire))
}

fn emit_wires(snapshot: &SchematicSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
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

fn emit_junctions(snapshot: &SchematicSnapshot, scene: &mut Scene) {
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

fn emit_arcs(snapshot: &SchematicSnapshot, scene: &mut Scene) {
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

fn emit_polygons(snapshot: &SchematicSnapshot, scene: &mut Scene) {
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

fn emit_text_bucket(texts: &[TextInput], output: &mut Vec<TextItem>) {
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

fn to_text_h_align(h_align: HAlign) -> TextHAlign {
    match h_align {
        HAlign::Left => TextHAlign::Left,
        HAlign::Center => TextHAlign::Center,
        HAlign::Right => TextHAlign::Right,
    }
}

fn to_text_v_align(v_align: VAlign) -> TextVAlign {
    match v_align {
        VAlign::Top => TextVAlign::Top,
        VAlign::Center => TextVAlign::Center,
        VAlign::Bottom => TextVAlign::Bottom,
    }
}

fn emit_texts(snapshot: &SchematicSnapshot, scene: &mut Scene) {
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

fn emit_overlay_line_bucket(lines: &[OverlayLineInput], output: &mut Vec<LineSegment>) {
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

fn emit_overlay_circle_bucket(circles: &[OverlayCircleInput], output: &mut Vec<Circle>) {
    for circle in circles {
        output.push(Circle {
            center: circle.center,
            radius: circle.radius_mm,
            stroke_width: circle.stroke_width_mm,
            color: circle.color,
        });
    }
}

fn emit_overlay_polygon_bucket(polygons: &[OverlayPolygonInput], output: &mut Vec<GpuPolygon>) {
    for polygon in polygons {
        output.push(GpuPolygon {
            vertices: polygon.vertices.clone(),
            fill_color: polygon.fill_color,
            stroke_color: polygon.stroke_color,
            stroke_width: polygon.stroke_width_mm,
        });
    }
}

fn emit_overlays(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.overlay_lines.clear();
    scene.overlay_circles.clear();
    scene.overlay_polygons.clear();

    scene.overlay_lines.reserve(
        snapshot.overlays.preview_lines.len() + snapshot.overlays.lasso_lines.len(),
    );
    scene
        .overlay_circles
        .reserve(snapshot.overlays.snap_circles.len());
    scene
        .overlay_polygons
        .reserve(snapshot.overlays.ghost_polygons.len());

    emit_overlay_line_bucket(&snapshot.overlays.preview_lines, &mut scene.overlay_lines);
    emit_overlay_polygon_bucket(&snapshot.overlays.ghost_polygons, &mut scene.overlay_polygons);
    emit_overlay_line_bucket(&snapshot.overlays.lasso_lines, &mut scene.overlay_lines);
    emit_overlay_circle_bucket(&snapshot.overlays.snap_circles, &mut scene.overlay_circles);
}

fn erc_style_ref(severity: Severity) -> StyleRef {
    let slot = match severity {
        Severity::Error => ColorSlot::ErcError,
        Severity::Warning => ColorSlot::ErcWarning,
        Severity::Info => ColorSlot::ErcInfo,
    };

    StyleRef::new(slot)
}

fn erc_color_from_style(style: StyleRef, theme: &ResolvedTheme) -> [f32; 4] {
    let slot = match style.slot {
        slot if slot == ColorSlot::ErcError as u16 => ColorSlot::ErcError,
        slot if slot == ColorSlot::ErcWarning as u16 => ColorSlot::ErcWarning,
        _ => ColorSlot::ErcInfo,
    };

    let mut color = theme.color(slot);

    color[3] = (color[3] * style.alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    color
}

fn erc_marker_vertices(severity: Severity, center: [f32; 2], radius_mm: f32) -> Vec<[f32; 2]> {
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

fn erc_marker_line(marker: &ErcMarkerInput) -> ([f32; 2], [f32; 2]) {
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

fn emit_erc_markers(snapshot: &SchematicSnapshot, theme: &ResolvedTheme, scene: &mut Scene) {
    scene.erc_marker_lines.clear();
    scene.erc_marker_circles.clear();
    scene.erc_marker_polygons.clear();

    scene.erc_marker_lines.reserve(snapshot.erc_markers.len());
    scene.erc_marker_circles.reserve(snapshot.erc_markers.len());
    scene.erc_marker_polygons.reserve(snapshot.erc_markers.len());

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

/// Phase-0 schematic renderer placeholder.
pub struct SchematicRenderer;

impl ViewRenderer for SchematicRenderer {
    type Snapshot = SchematicSnapshot;

    fn build_scene(
        snapshot: &Self::Snapshot,
        theme: &ResolvedTheme,
        dirty: DirtyFlags,
        scene: &mut Scene,
    ) {
        if dirty.contains(DirtyFlags::LINES) {
            emit_wires(snapshot, theme, scene);
        }

        if dirty.contains(DirtyFlags::CIRCLES) {
            emit_junctions(snapshot, scene);
        }

        if dirty.contains(DirtyFlags::ARCS) {
            emit_arcs(snapshot, scene);
        }

        if dirty.contains(DirtyFlags::POLYGONS) {
            emit_polygons(snapshot, scene);
        }

        if dirty.contains(DirtyFlags::TEXT) {
            emit_texts(snapshot, scene);
        }

        if dirty.contains(DirtyFlags::OVERLAY) {
            emit_overlays(snapshot, scene);
            emit_erc_markers(snapshot, theme, scene);
        }

        scene.dirty |= dirty;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArcInput, ErcMarkerInput, OverlayCircleInput, OverlayInputs, OverlayLineInput,
        OverlayPolygonInput, PolygonInput, SchematicRenderer, SchematicSnapshot, TextInput,
        ViewRenderer, WireInput,
    };
    use signex_gfx::primitive::text::{TextHAlign, TextVAlign};
    use signex_gfx::scene::{DirtyFlags, Scene};
    use signex_gfx::style::ColorSlot;
    use signex_types::schematic::{HAlign, VAlign};
    use signex_types::violation::Severity;
    use std::collections::HashMap;

    use crate::theme::ResolvedTheme;

    fn make_wire(id: u64, explicit_color: Option<[f32; 4]>) -> WireInput {
        WireInput {
            id,
            p0: [0.0, 0.0],
            p1: [5.0, 0.0],
            width_mm: 0.15,
            explicit_color,
        }
    }

    fn make_arc(radius_mm: f32, start_angle_rad: f32, end_angle_rad: f32) -> ArcInput {
        ArcInput {
            center: [3.0, 2.0],
            radius_mm,
            start_angle_rad,
            end_angle_rad,
            width_mm: 0.18,
            color: [0.9, 0.5, 0.2, 1.0],
        }
    }

    fn make_polygon() -> PolygonInput {
        PolygonInput {
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.2, 0.6, 0.9, 1.0],
            stroke_color: Some([0.1, 0.1, 0.1, 1.0]),
            stroke_width_mm: 0.15,
        }
    }

    fn make_text(content: &str, rotation_rad: f32) -> TextInput {
        TextInput {
            content: content.to_string(),
            position: [3.0, 3.0],
            size_mm: 1.1,
            color: [0.95, 0.95, 0.95, 1.0],
            bold: true,
            italic: false,
            rotation_rad,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
        }
    }

    fn make_overlay_line(p0: [f32; 2], p1: [f32; 2], color: [f32; 4]) -> OverlayLineInput {
        OverlayLineInput {
            p0,
            p1,
            width_mm: 0.12,
            color,
        }
    }

    fn make_overlay_polygon(color: [f32; 4]) -> OverlayPolygonInput {
        OverlayPolygonInput {
            vertices: vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
            fill_color: color,
            stroke_color: Some([color[0], color[1], color[2], 1.0]),
            stroke_width_mm: 0.1,
        }
    }

    fn make_overlay_circle(center: [f32; 2], color: [f32; 4]) -> OverlayCircleInput {
        OverlayCircleInput {
            center,
            radius_mm: 0.2,
            stroke_width_mm: 0.08,
            color,
        }
    }

    fn default_theme() -> ResolvedTheme {
        ResolvedTheme::builtin_default()
    }

    fn build_scene_with_default_theme(
        snapshot: &SchematicSnapshot,
        dirty: DirtyFlags,
        scene: &mut Scene,
    ) {
        let theme = default_theme();
        SchematicRenderer::build_scene(snapshot, &theme, dirty, scene);
    }

    fn make_erc_marker(center: [f32; 2], radius_mm: f32, severity: Severity) -> ErcMarkerInput {
        ErcMarkerInput {
            center,
            radius_mm,
            severity,
        }
    }

    #[test]
    fn wire_color_order_prefers_override_then_explicit_then_theme() {
        let mut overrides = HashMap::new();
        overrides.insert(1, [0.8, 0.1, 0.1, 1.0]);

        let snapshot = SchematicSnapshot {
            wires: vec![
                make_wire(1, Some([0.1, 0.8, 0.1, 1.0])),
                make_wire(2, Some([0.1, 0.8, 0.1, 1.0])),
                make_wire(3, None),
            ],
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: overrides,
        };

        let theme = default_theme().with_slot(ColorSlot::Wire, [0.2, 0.2, 0.9, 1.0]);
        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);

        assert_eq!(scene.lines.len(), 3);
        assert_eq!(scene.lines[0].color, [0.8, 0.1, 0.1, 1.0]);
        assert_eq!(scene.lines[1].color, [0.1, 0.8, 0.1, 1.0]);
        assert_eq!(scene.lines[2].color, [0.2, 0.2, 0.9, 1.0]);
    }

    #[test]
    fn build_scene_updates_requested_primitive_groups_only() {
        let snapshot = SchematicSnapshot {
            wires: vec![make_wire(7, Some([0.7, 0.7, 0.7, 1.0]))],
            junctions: vec![super::JunctionInput {
                center: [1.0, 1.0],
                radius_mm: 0.3,
                color: [0.9, 0.9, 0.2, 1.0],
            }],
            arcs: vec![make_arc(1.2, 0.0, 1.5707964)],
            polygons: vec![make_polygon()],
            labels: vec![make_text("R1", 0.0)],
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs {
                preview_lines: vec![make_overlay_line([0.0, 5.0], [4.0, 5.0], [0.7, 0.7, 1.0, 0.7])],
                ghost_polygons: vec![make_overlay_polygon([0.4, 0.6, 1.0, 0.3])],
                lasso_lines: vec![make_overlay_line([1.0, 1.0], [1.0, 4.0], [0.9, 0.9, 0.3, 1.0])],
                snap_circles: vec![make_overlay_circle([2.0, 2.0], [0.2, 0.9, 0.9, 1.0])],
            },
            erc_markers: vec![make_erc_marker([2.8, 2.8], 0.25, Severity::Warning)],
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::LINES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 0);
        assert_eq!(scene.arcs.len(), 0);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);
        assert_eq!(scene.overlay_lines.len(), 0);
        assert_eq!(scene.overlay_circles.len(), 0);
        assert_eq!(scene.overlay_polygons.len(), 0);
        assert_eq!(scene.erc_marker_lines.len(), 0);
        assert_eq!(scene.erc_marker_circles.len(), 0);
        assert_eq!(scene.erc_marker_polygons.len(), 0);

        build_scene_with_default_theme(&snapshot, DirtyFlags::CIRCLES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.arcs.len(), 0);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);
        assert_eq!(scene.overlay_lines.len(), 0);
        assert_eq!(scene.overlay_circles.len(), 0);
        assert_eq!(scene.overlay_polygons.len(), 0);
        assert_eq!(scene.erc_marker_lines.len(), 0);
        assert_eq!(scene.erc_marker_circles.len(), 0);
        assert_eq!(scene.erc_marker_polygons.len(), 0);

        build_scene_with_default_theme(&snapshot, DirtyFlags::ARCS, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.arcs.len(), 1);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);
        assert_eq!(scene.overlay_lines.len(), 0);
        assert_eq!(scene.overlay_circles.len(), 0);
        assert_eq!(scene.overlay_polygons.len(), 0);
        assert_eq!(scene.erc_marker_lines.len(), 0);
        assert_eq!(scene.erc_marker_circles.len(), 0);
        assert_eq!(scene.erc_marker_polygons.len(), 0);

        build_scene_with_default_theme(&snapshot, DirtyFlags::POLYGONS, &mut scene);
        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.texts.len(), 0);

        build_scene_with_default_theme(&snapshot, DirtyFlags::TEXT, &mut scene);
        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.texts.len(), 1);
        assert_eq!(scene.overlay_lines.len(), 0);
        assert_eq!(scene.overlay_circles.len(), 0);
        assert_eq!(scene.overlay_polygons.len(), 0);
        assert_eq!(scene.erc_marker_lines.len(), 0);
        assert_eq!(scene.erc_marker_circles.len(), 0);
        assert_eq!(scene.erc_marker_polygons.len(), 0);

        build_scene_with_default_theme(&snapshot, DirtyFlags::OVERLAY, &mut scene);
        assert_eq!(scene.overlay_lines.len(), 2);
        assert_eq!(scene.overlay_circles.len(), 1);
        assert_eq!(scene.overlay_polygons.len(), 1);
        assert_eq!(scene.erc_marker_lines.len(), 1);
        assert_eq!(scene.erc_marker_circles.len(), 1);
        assert_eq!(scene.erc_marker_polygons.len(), 1);
        assert!(scene.dirty.contains(DirtyFlags::LINES));
        assert!(scene.dirty.contains(DirtyFlags::CIRCLES));
        assert!(scene.dirty.contains(DirtyFlags::ARCS));
        assert!(scene.dirty.contains(DirtyFlags::POLYGONS));
        assert!(scene.dirty.contains(DirtyFlags::TEXT));
        assert!(scene.dirty.contains(DirtyFlags::OVERLAY));
    }

    #[test]
    fn arc_emitter_preserves_wraparound_and_tiny_radius_inputs() {
        let wrap_start = 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_6;
        let wrap_end = std::f32::consts::FRAC_PI_6;

        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: vec![
                make_arc(2.0, wrap_start, wrap_end),
                make_arc(0.01, 0.0, 1.5707964),
            ],
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::ARCS, &mut scene);

        assert_eq!(scene.lines.len(), 0);
        assert_eq!(scene.circles.len(), 0);
        assert_eq!(scene.arcs.len(), 2);
        assert_eq!(scene.arcs[0].start_angle, wrap_start);
        assert_eq!(scene.arcs[0].end_angle, wrap_end);
        assert_eq!(scene.arcs[1].radius, 0.01);
    }

    #[test]
    fn polygon_text_emitters_preserve_mapped_fields() {
        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: vec![make_polygon()],
            labels: vec![make_text("VIN", -std::f32::consts::FRAC_PI_2)],
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::POLYGONS | DirtyFlags::TEXT, &mut scene);

        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.polygons[0].vertices.len(), 4);
        assert_eq!(scene.polygons[0].fill_color, [0.2, 0.6, 0.9, 1.0]);
        assert_eq!(scene.polygons[0].stroke_width, 0.15);
        assert_eq!(scene.texts.len(), 1);
        assert_eq!(scene.texts[0].content, "VIN");
        assert_eq!(scene.texts[0].position, [3.0, 3.0]);
        assert_eq!(scene.texts[0].size_mm, 1.1);
        assert_eq!(scene.texts[0].rotation, -std::f32::consts::FRAC_PI_2);
        assert_eq!(scene.texts[0].h_align, TextHAlign::Left);
        assert_eq!(scene.texts[0].v_align, TextVAlign::Top);
    }

    #[test]
    fn text_emitter_covers_all_text_categories() {
        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: vec![make_text("NET_A", 0.0)],
            pin_texts: vec![make_text("PA0", 0.0)],
            reference_value_texts: vec![make_text("R3", 0.0), make_text("10k", 0.0)],
            parameter_texts: vec![make_text("Tolerance=1%", 0.0)],
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::TEXT, &mut scene);

        assert_eq!(scene.texts.len(), 5);
        assert_eq!(scene.texts[0].content, "NET_A");
        assert_eq!(scene.texts[1].content, "PA0");
        assert_eq!(scene.texts[2].content, "R3");
        assert_eq!(scene.texts[3].content, "10k");
        assert_eq!(scene.texts[4].content, "Tolerance=1%");
        assert!(scene.dirty.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn text_emitter_maps_alignment_variants() {
        let mut centered = make_text("CENTERED", 0.0);
        centered.h_align = HAlign::Center;
        centered.v_align = VAlign::Center;

        let mut right_bottom = make_text("RB", std::f32::consts::FRAC_PI_2);
        right_bottom.h_align = HAlign::Right;
        right_bottom.v_align = VAlign::Bottom;

        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: vec![centered],
            pin_texts: vec![right_bottom],
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::TEXT, &mut scene);

        assert_eq!(scene.texts.len(), 2);
        assert_eq!(scene.texts[0].h_align, TextHAlign::Center);
        assert_eq!(scene.texts[0].v_align, TextVAlign::Center);
        assert_eq!(scene.texts[1].h_align, TextHAlign::Right);
        assert_eq!(scene.texts[1].v_align, TextVAlign::Bottom);
        assert_eq!(scene.texts[1].rotation, std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn overlay_emitter_maps_preview_ghost_lasso_snap() {
        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs {
                preview_lines: vec![make_overlay_line([0.0, 0.0], [2.0, 0.0], [0.6, 0.6, 1.0, 0.8])],
                ghost_polygons: vec![make_overlay_polygon([0.5, 0.8, 1.0, 0.3])],
                lasso_lines: vec![
                    make_overlay_line([1.0, 1.0], [3.0, 1.0], [0.9, 0.9, 0.4, 1.0]),
                    make_overlay_line([3.0, 1.0], [3.0, 3.0], [0.9, 0.9, 0.4, 1.0]),
                ],
                snap_circles: vec![make_overlay_circle([2.0, 2.0], [0.2, 1.0, 0.9, 1.0])],
            },
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::OVERLAY, &mut scene);

        assert_eq!(scene.overlay_lines.len(), 3);
        assert_eq!(scene.overlay_polygons.len(), 1);
        assert_eq!(scene.overlay_circles.len(), 1);
        assert_eq!(scene.overlay_lines[0].color, [0.6, 0.6, 1.0, 0.8]);
        assert_eq!(scene.overlay_lines[1].color, [0.9, 0.9, 0.4, 1.0]);
        assert_eq!(scene.overlay_polygons[0].fill_color, [0.5, 0.8, 1.0, 0.3]);
        assert_eq!(scene.overlay_circles[0].center, [2.0, 2.0]);
        assert_eq!(scene.erc_marker_lines.len(), 0);
        assert_eq!(scene.erc_marker_circles.len(), 0);
        assert_eq!(scene.erc_marker_polygons.len(), 0);
        assert!(scene.dirty.contains(DirtyFlags::OVERLAY));
    }

    #[test]
    fn erc_marker_emitter_maps_severity_slots_to_palette_colors() {
        let theme = default_theme()
            .with_slot(ColorSlot::ErcError, [0.91, 0.2, 0.2, 1.0])
            .with_slot(ColorSlot::ErcWarning, [0.96, 0.72, 0.2, 1.0])
            .with_slot(ColorSlot::ErcInfo, [0.26, 0.67, 0.94, 1.0]);

        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: vec![
                make_erc_marker([1.0, 1.0], 0.25, Severity::Error),
                make_erc_marker([2.0, 1.0], 0.25, Severity::Warning),
                make_erc_marker([3.0, 1.0], 0.25, Severity::Info),
            ],
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);

        assert_eq!(scene.erc_marker_lines.len(), 3);
        assert_eq!(scene.erc_marker_circles.len(), 3);
        assert_eq!(scene.erc_marker_polygons.len(), 3);

        assert_eq!(scene.erc_marker_lines[0].color, [0.91, 0.2, 0.2, 1.0]);
        assert_eq!(scene.erc_marker_lines[1].color, [0.96, 0.72, 0.2, 1.0]);
        assert_eq!(scene.erc_marker_lines[2].color, [0.26, 0.67, 0.94, 1.0]);
        assert_eq!(scene.erc_marker_polygons[0].vertices.len(), 3);
        assert_eq!(scene.erc_marker_polygons[1].vertices.len(), 4);
        assert_eq!(scene.erc_marker_polygons[2].vertices.len(), 4);
        assert!(scene.dirty.contains(DirtyFlags::OVERLAY));
    }

    #[test]
    fn erc_marker_emitter_handles_dense_marker_cluster() {
        let markers = vec![
            make_erc_marker([4.0, 4.0], 0.2, Severity::Error),
            make_erc_marker([4.1, 4.0], 0.2, Severity::Warning),
            make_erc_marker([4.2, 4.0], 0.2, Severity::Info),
            make_erc_marker([4.0, 4.1], 0.2, Severity::Error),
            make_erc_marker([4.1, 4.1], 0.2, Severity::Warning),
            make_erc_marker([4.2, 4.1], 0.2, Severity::Info),
        ];

        let snapshot = SchematicSnapshot {
            wires: Vec::new(),
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: markers,
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        build_scene_with_default_theme(&snapshot, DirtyFlags::OVERLAY, &mut scene);

        assert_eq!(scene.erc_marker_lines.len(), 6);
        assert_eq!(scene.erc_marker_circles.len(), 6);
        assert_eq!(scene.erc_marker_polygons.len(), 6);
        assert!(scene.erc_marker_lines.iter().all(|line| line.width > 0.0));
        assert!(scene.erc_marker_circles.iter().all(|circle| circle.radius > 0.0));
        assert!(scene
            .erc_marker_polygons
            .iter()
            .all(|polygon| polygon.vertices.len() >= 3));
        assert!(scene.dirty.contains(DirtyFlags::OVERLAY));
    }

    #[test]
    fn theme_resolved_flow_updates_wire_and_erc_colors() {
        let theme = default_theme()
            .with_slot(ColorSlot::Wire, [0.12, 0.34, 0.56, 1.0])
            .with_slot(ColorSlot::ErcError, [0.91, 0.11, 0.21, 1.0]);

        let snapshot = SchematicSnapshot {
            wires: vec![make_wire(42, None)],
            junctions: Vec::new(),
            arcs: Vec::new(),
            polygons: Vec::new(),
            labels: Vec::new(),
            pin_texts: Vec::new(),
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: vec![make_erc_marker([1.0, 1.0], 0.25, Severity::Error)],
            wire_color_overrides: HashMap::new(),
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(
            &snapshot,
            &theme,
            DirtyFlags::LINES | DirtyFlags::OVERLAY,
            &mut scene,
        );

        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.lines[0].color, [0.12, 0.34, 0.56, 1.0]);
        assert_eq!(scene.erc_marker_lines.len(), 1);
        assert_eq!(scene.erc_marker_lines[0].color, [0.91, 0.11, 0.21, 1.0]);
    }
}
