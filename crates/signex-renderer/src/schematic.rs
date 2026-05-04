//! Schematic renderer interfaces.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use signex_gfx::scene::{DirtyFlags, Scene};
use std::collections::HashMap;

use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::TextItem;

/// Common view renderer contract used by scene translators.
pub trait ViewRenderer {
    type Snapshot;

    fn build_scene(snapshot: &Self::Snapshot, dirty: DirtyFlags, scene: &mut Scene);
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
}

#[derive(Clone, Debug)]
pub struct SchematicSnapshot {
    pub wires: Vec<WireInput>,
    pub junctions: Vec<JunctionInput>,
    pub arcs: Vec<ArcInput>,
    pub polygons: Vec<PolygonInput>,
    pub texts: Vec<TextInput>,
    pub wire_color_overrides: HashMap<u64, [f32; 4]>,
    pub wire_theme_default: [f32; 4],
}

fn resolve_wire_color(wire: &WireInput, snapshot: &SchematicSnapshot) -> [f32; 4] {
    snapshot
        .wire_color_overrides
        .get(&wire.id)
        .copied()
        .or(wire.explicit_color)
        .unwrap_or(snapshot.wire_theme_default)
}

fn emit_wires(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.lines.clear();
    scene.lines.reserve(snapshot.wires.len());

    for wire in &snapshot.wires {
        scene.lines.push(LineSegment {
            p0: wire.p0,
            p1: wire.p1,
            width: wire.width_mm,
            color: resolve_wire_color(wire, snapshot),
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

fn emit_texts(snapshot: &SchematicSnapshot, scene: &mut Scene) {
    scene.texts.clear();
    scene.texts.reserve(snapshot.texts.len());

    for text in &snapshot.texts {
        scene.texts.push(TextItem {
            content: text.content.clone(),
            position: text.position,
            size_mm: text.size_mm,
            color: text.color,
            bold: text.bold,
            italic: text.italic,
            rotation: text.rotation_rad,
        });
    }
}

/// Phase-0 schematic renderer placeholder.
pub struct SchematicRenderer;

impl ViewRenderer for SchematicRenderer {
    type Snapshot = SchematicSnapshot;

    fn build_scene(snapshot: &Self::Snapshot, dirty: DirtyFlags, scene: &mut Scene) {
        if dirty.contains(DirtyFlags::LINES) {
            emit_wires(snapshot, scene);
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

        scene.dirty |= dirty;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArcInput, PolygonInput, SchematicRenderer, SchematicSnapshot, TextInput, ViewRenderer,
        WireInput,
    };
    use signex_gfx::scene::{DirtyFlags, Scene};
    use std::collections::HashMap;

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
            texts: Vec::new(),
            wire_color_overrides: overrides,
            wire_theme_default: [0.2, 0.2, 0.9, 1.0],
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, DirtyFlags::LINES, &mut scene);

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
            texts: vec![make_text("R1", 0.0)],
            wire_color_overrides: HashMap::new(),
            wire_theme_default: [0.1, 0.1, 0.1, 1.0],
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, DirtyFlags::LINES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 0);
        assert_eq!(scene.arcs.len(), 0);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);

        SchematicRenderer::build_scene(&snapshot, DirtyFlags::CIRCLES, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.arcs.len(), 0);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);

        SchematicRenderer::build_scene(&snapshot, DirtyFlags::ARCS, &mut scene);
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.circles.len(), 1);
        assert_eq!(scene.arcs.len(), 1);
        assert_eq!(scene.polygons.len(), 0);
        assert_eq!(scene.texts.len(), 0);

        SchematicRenderer::build_scene(&snapshot, DirtyFlags::POLYGONS, &mut scene);
        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.texts.len(), 0);

        SchematicRenderer::build_scene(&snapshot, DirtyFlags::TEXT, &mut scene);
        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.texts.len(), 1);
        assert!(scene.dirty.contains(DirtyFlags::LINES));
        assert!(scene.dirty.contains(DirtyFlags::CIRCLES));
        assert!(scene.dirty.contains(DirtyFlags::ARCS));
        assert!(scene.dirty.contains(DirtyFlags::POLYGONS));
        assert!(scene.dirty.contains(DirtyFlags::TEXT));
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
            texts: Vec::new(),
            wire_color_overrides: HashMap::new(),
            wire_theme_default: [0.1, 0.1, 0.1, 1.0],
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, DirtyFlags::ARCS, &mut scene);

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
            texts: vec![make_text("VIN", -std::f32::consts::FRAC_PI_2)],
            wire_color_overrides: HashMap::new(),
            wire_theme_default: [0.1, 0.1, 0.1, 1.0],
        };

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(&snapshot, DirtyFlags::POLYGONS | DirtyFlags::TEXT, &mut scene);

        assert_eq!(scene.polygons.len(), 1);
        assert_eq!(scene.polygons[0].vertices.len(), 4);
        assert_eq!(scene.polygons[0].fill_color, [0.2, 0.6, 0.9, 1.0]);
        assert_eq!(scene.polygons[0].stroke_width, 0.15);
        assert_eq!(scene.texts.len(), 1);
        assert_eq!(scene.texts[0].content, "VIN");
        assert_eq!(scene.texts[0].position, [3.0, 3.0]);
        assert_eq!(scene.texts[0].size_mm, 1.1);
        assert_eq!(scene.texts[0].rotation, -std::f32::consts::FRAC_PI_2);
    }
}
