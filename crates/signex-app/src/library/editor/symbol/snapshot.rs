//! Symbol → `SchematicSnapshot` mapper — groundwork for routing the
//! symbol editor through dev's unified scene renderer (`signex-gfx` /
//! `signex-renderer`), salvaged from `feature/v0.13-symbol`.
//!
//! # Status: groundwork, NOT wired into the live canvas
//!
//! `build_symbol_snapshot` produces the same [`SchematicSnapshot`] the
//! schematic renderer consumes (each symbol primitive mapped to a
//! schematic input: rect→polygon, line→wire, circle→polygon, arc→arc,
//! text→label, pin→wire+junction+halo+2 texts). It is fully
//! unit-tested but nothing calls it yet.
//!
//! To activate (the remaining, visual-verification-gated work):
//! 1. Add scene drawers for `scene.arcs` and `scene.texts` — dev's
//!    `pcb_canvas::draw_scene` only handles lines/circles/polygons.
//! 2. Swap [`super::canvas::SymbolCanvas::draw`]'s primitive loop for
//!    `Scene::default()` → `SchematicRenderer::build_scene(&snapshot,
//!    &theme, dirty, &mut scene)` → `draw_scene(..)`, keeping the grid,
//!    crosshair, resize handles and rubber-band overlays as they are.
//! 3. Visually confirm stroke widths / arc sweeps / text placement in
//!    the running app.
//!
//! See `docs/audit/symbol-shader-renderer-salvage.md` for the full plan.

use iced::Color;
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};
use signex_renderer::schematic::{
    ArcInput, JunctionInput, OverlayInputs, PolygonInput, SchematicSnapshot, TextInput, WireInput,
};
use signex_types::schematic::{HAlign, VAlign};

use super::state::{pin_body_delta, PinTextGeometry, SymbolSelection};

// Stroke widths in screen px at 100% zoom (ported from
// feature/v0.13-symbol). `stroke_world_mm` converts them to the
// world-mm widths the snapshot carries.
const GRAPHIC_STROKE_PX: f32 = 1.5;
const GRAPHIC_SELECTED_STROKE_PX: f32 = 2.5;
const RECT_STROKE_PX: f32 = 2.0;
const RECT_SELECTED_STROKE_PX: f32 = 2.5;
const PIN_STROKE_PX: f32 = 1.5;
const PIN_SELECTED_STROKE_PX: f32 = 2.5;
const PIN_HALO_STROKE_PX: f32 = 1.0;
/// dev lacks `signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM`,
/// so the floor is module-local.
const MIN_STROKE_MM: f32 = 0.05;
const PIN_NUMBER_SIZE_MM: f32 = 1.2;
const PIN_NAME_SIZE_MM: f32 = 1.2;
const BODY_FILL_ALPHA: f32 = 0.16;
const CIRCLE_SEGMENTS: usize = 40;

/// The four theme colours the snapshot needs, passed explicitly so the
/// builder stays independent of the canvas widget (and unit-testable).
#[derive(Debug, Clone, Copy)]
pub struct SymbolSnapshotColors {
    pub body: Color,
    pub selected: Color,
    pub pin: Color,
    pub text: Color,
}

fn to_rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

/// A stroke that is `px_at_100` pixels wide at 100% zoom, expressed as
/// the world-mm width the renderer expects, floored so it never
/// vanishes at high zoom-out.
fn stroke_world_mm(px_at_100: f32, scale: f32) -> f32 {
    (px_at_100 / scale.max(0.001)).max(MIN_STROKE_MM)
}

fn screen_px_to_world_mm(px: f32, scale: f32) -> f32 {
    (px / scale.max(0.001)).max(0.01)
}

/// Tessellate a circle into `segments` world-space vertices (for the
/// snapshot's polygon-only circle representation).
fn circle_vertices(center: [f64; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let n = segments.max(12);
    let (cx, cy) = (center[0] as f32, center[1] as f32);
    let r = radius.max(0.01);
    (0..n)
        .map(|i| {
            let t = (i as f32 / n as f32) * std::f32::consts::TAU;
            [cx + r * t.cos(), cy + r * t.sin()]
        })
        .collect()
}

fn pin_is_selected(sel: &Option<SymbolSelection>, i: usize) -> bool {
    match sel {
        Some(SymbolSelection::Pin(j)) => *j == i,
        Some(SymbolSelection::All) => true,
        Some(SymbolSelection::Multiple { pin_indices, .. }) => pin_indices.contains(&i),
        _ => false,
    }
}

fn graphic_is_selected(sel: &Option<SymbolSelection>, i: usize) -> bool {
    match sel {
        Some(SymbolSelection::Graphic(j)) => *j == i,
        Some(SymbolSelection::All) => true,
        Some(SymbolSelection::Multiple {
            graphic_indices, ..
        }) => graphic_indices.contains(&i),
        _ => false,
    }
}

/// Pins on Part Zero (`part_number == 0`) render on every part; others
/// only on the active part. Mirrors `SymbolCanvas::pin_visible_on_active_part`.
fn pin_on_active_part(pin: &SymbolPin, active_part: u8) -> bool {
    pin.part_number == 0 || pin.part_number == active_part
}

/// Build the renderer snapshot for `symbol`. Pure data mapping — no
/// canvas / GPU state. See the module docs for the activation plan.
pub fn build_symbol_snapshot(
    symbol: &Symbol,
    selected: &Option<SymbolSelection>,
    active_part: u8,
    colors: &SymbolSnapshotColors,
    scale: f32,
) -> SchematicSnapshot {
    let mut wires = Vec::new();
    let mut junctions = Vec::new();
    let mut arcs = Vec::new();
    let mut polygons = Vec::new();
    let mut labels = Vec::new();
    let mut pin_texts = Vec::new();

    // The first rectangle is the filled body; later graphics are
    // outline-only (matches the iced draw path).
    let mut body_drawn = false;
    for (i, g) in symbol.graphics.iter().enumerate() {
        let is_sel = graphic_is_selected(selected, i);
        let stroke = if is_sel { colors.selected } else { colors.body };
        let sw = stroke_world_mm(
            if is_sel {
                GRAPHIC_SELECTED_STROKE_PX
            } else {
                GRAPHIC_STROKE_PX
            },
            scale,
        );
        let rw = stroke_world_mm(
            if is_sel {
                RECT_SELECTED_STROKE_PX
            } else {
                RECT_STROKE_PX
            },
            scale,
        );
        match &g.kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                let (x0, y0, x1, y1) = (from[0] as f32, from[1] as f32, to[0] as f32, to[1] as f32);
                let fill = if !body_drawn {
                    body_drawn = true;
                    to_rgba(Color {
                        a: BODY_FILL_ALPHA,
                        ..colors.body
                    })
                } else {
                    [0.0; 4]
                };
                polygons.push(PolygonInput {
                    vertices: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                    fill_color: fill,
                    stroke_color: Some(to_rgba(stroke)),
                    stroke_width_mm: rw,
                });
            }
            SymbolGraphicKind::Line { from, to } => {
                wires.push(WireInput {
                    id: i as u64,
                    p0: [from[0] as f32, from[1] as f32],
                    p1: [to[0] as f32, to[1] as f32],
                    width_mm: sw,
                    explicit_color: Some(to_rgba(stroke)),
                });
            }
            SymbolGraphicKind::Circle { center, radius } => {
                polygons.push(PolygonInput {
                    vertices: circle_vertices(*center, *radius as f32, CIRCLE_SEGMENTS),
                    fill_color: [0.0; 4],
                    stroke_color: Some(to_rgba(stroke)),
                    stroke_width_mm: sw,
                });
            }
            SymbolGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                arcs.push(ArcInput {
                    center: [center[0] as f32, center[1] as f32],
                    radius_mm: *radius as f32,
                    start_angle_rad: (*start_deg as f32).to_radians(),
                    end_angle_rad: (*end_deg as f32).to_radians(),
                    width_mm: sw,
                    color: to_rgba(stroke),
                });
            }
            SymbolGraphicKind::Text {
                position,
                content,
                size,
            } => {
                labels.push(TextInput {
                    content: content.clone(),
                    position: [position[0] as f32, position[1] as f32],
                    size_mm: (*size as f32).max(0.1),
                    color: to_rgba(if is_sel { colors.selected } else { colors.text }),
                    bold: false,
                    italic: false,
                    rotation_rad: 0.0,
                    h_align: HAlign::Left,
                    v_align: VAlign::Top,
                });
            }
        }
    }

    for (i, pin) in symbol.pins.iter().enumerate() {
        if !pin_on_active_part(pin, active_part) {
            continue;
        }
        let is_sel = pin_is_selected(selected, i);
        let stroke = if is_sel { colors.selected } else { colors.pin };
        let (dx, dy) = pin_body_delta(pin);
        let tip = [pin.position[0] as f32, pin.position[1] as f32];
        let body_end = [(pin.position[0] + dx) as f32, (pin.position[1] + dy) as f32];

        // Pin stub line (tip → body-end).
        wires.push(WireInput {
            id: 100_000 + i as u64,
            p0: tip,
            p1: body_end,
            width_mm: stroke_world_mm(
                if is_sel {
                    PIN_SELECTED_STROKE_PX
                } else {
                    PIN_STROKE_PX
                },
                scale,
            ),
            explicit_color: Some(to_rgba(stroke)),
        });

        // Connection dot at the tip.
        junctions.push(JunctionInput {
            center: tip,
            radius_mm: screen_px_to_world_mm(2.5, scale),
            color: to_rgba(stroke),
        });

        // Selection halo ring around the tip.
        if is_sel {
            polygons.push(PolygonInput {
                vertices: circle_vertices(
                    pin.position,
                    screen_px_to_world_mm(5.0, scale),
                    CIRCLE_SEGMENTS,
                ),
                fill_color: [0.0; 4],
                stroke_color: Some(to_rgba(colors.selected)),
                stroke_width_mm: stroke_world_mm(PIN_HALO_STROKE_PX, scale),
            });
        }

        // Number (over the stub midpoint) + name (past the body-end),
        // rotated to run along the pin. `name_flipped` reverses the
        // name's horizontal anchor for Left-facing pins.
        let tg = PinTextGeometry::compute(pin.orientation);
        let name_align = if tg.name_flipped {
            HAlign::Right
        } else {
            HAlign::Left
        };
        let mid = [(tip[0] + body_end[0]) * 0.5, (tip[1] + body_end[1]) * 0.5];
        pin_texts.push(TextInput {
            content: pin.number.clone(),
            position: mid,
            size_mm: PIN_NUMBER_SIZE_MM,
            color: to_rgba(colors.text),
            bold: false,
            italic: false,
            rotation_rad: tg.text_rotation,
            h_align: HAlign::Center,
            v_align: VAlign::Bottom,
        });
        pin_texts.push(TextInput {
            content: pin.name.clone(),
            position: body_end,
            size_mm: PIN_NAME_SIZE_MM,
            color: to_rgba(Color {
                a: 0.85,
                ..colors.text
            }),
            bold: false,
            italic: false,
            rotation_rad: tg.text_rotation,
            h_align: name_align,
            v_align: VAlign::Center,
        });
    }

    SchematicSnapshot {
        wires,
        junctions,
        arcs,
        polygons,
        labels,
        pin_texts,
        reference_value_texts: Vec::new(),
        parameter_texts: Vec::new(),
        overlays: OverlayInputs::default(),
        erc_markers: Vec::new(),
        wire_color_overrides: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{PinOrientation, SymbolGraphic};

    fn colors() -> SymbolSnapshotColors {
        SymbolSnapshotColors {
            body: Color::WHITE,
            selected: Color::from_rgb(1.0, 0.0, 0.0),
            pin: Color::from_rgb(0.0, 1.0, 0.0),
            text: Color::from_rgb(0.0, 0.0, 1.0),
        }
    }

    fn empty() -> Symbol {
        let mut s = Symbol::empty("U1");
        s.pins.clear();
        s.graphics.clear();
        s
    }

    #[test]
    fn rectangle_maps_to_a_four_vertex_polygon_with_body_fill() {
        let mut s = empty();
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [10.0, 5.0],
            },
            stroke_width: 0.15,
        });
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.polygons.len(), 1);
        let p = &snap.polygons[0];
        assert_eq!(p.vertices.len(), 4);
        // First rectangle is the filled body → non-zero fill alpha.
        assert!(p.fill_color[3] > 0.0);
        assert!(p.stroke_color.is_some());
    }

    #[test]
    fn line_maps_to_a_wire() {
        let mut s = empty();
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [5.0, 0.0],
            },
            stroke_width: 0.15,
        });
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.wires.len(), 1);
        assert_eq!(snap.wires[0].p0, [0.0, 0.0]);
        assert_eq!(snap.wires[0].p1, [5.0, 0.0]);
    }

    #[test]
    fn arc_maps_to_an_arc_with_radian_angles() {
        let mut s = empty();
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Arc {
                center: [1.0, 2.0],
                radius: 3.0,
                start_deg: 0.0,
                end_deg: 90.0,
            },
            stroke_width: 0.15,
        });
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.arcs.len(), 1);
        let a = &snap.arcs[0];
        assert_eq!(a.center, [1.0, 2.0]);
        assert_eq!(a.radius_mm, 3.0);
        assert!((a.start_angle_rad - 0.0).abs() < 1e-6);
        assert!((a.end_angle_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn circle_maps_to_a_tessellated_outline_polygon() {
        let mut s = empty();
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Circle {
                center: [0.0, 0.0],
                radius: 2.0,
            },
            stroke_width: 0.15,
        });
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.polygons.len(), 1);
        assert_eq!(snap.polygons[0].vertices.len(), CIRCLE_SEGMENTS);
        // Outline only — no fill.
        assert_eq!(snap.polygons[0].fill_color, [0.0; 4]);
    }

    #[test]
    fn text_maps_to_a_label() {
        let mut s = empty();
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Text {
                position: [1.0, 1.0],
                content: "R".into(),
                size: 1.5,
            },
            stroke_width: 0.0,
        });
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.labels.len(), 1);
        assert_eq!(snap.labels[0].content, "R");
        assert_eq!(snap.labels[0].position, [1.0, 1.0]);
    }

    #[test]
    fn pin_maps_to_wire_plus_junction_plus_two_texts() {
        let mut s = empty();
        let mut pin = SymbolPin::new("1", "VCC");
        pin.position = [0.0, 0.0];
        pin.orientation = PinOrientation::Right;
        pin.length = 2.54;
        s.pins.push(pin);
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.wires.len(), 1); // pin stub
        assert_eq!(snap.junctions.len(), 1); // tip dot
        assert_eq!(snap.pin_texts.len(), 2); // number + name
        assert_eq!(snap.pin_texts[0].content, "1");
        assert_eq!(snap.pin_texts[1].content, "VCC");
        // Right pin body extends to +x.
        assert_eq!(snap.wires[0].p1, [2.54, 0.0]);
    }

    #[test]
    fn selected_pin_adds_a_halo_polygon() {
        let mut s = empty();
        let mut pin = SymbolPin::new("1", "A");
        pin.position = [0.0, 0.0];
        s.pins.push(pin);
        let unsel = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        let sel = build_symbol_snapshot(&s, &Some(SymbolSelection::Pin(0)), 1, &colors(), 10.0);
        assert_eq!(unsel.polygons.len(), 0);
        assert_eq!(sel.polygons.len(), 1); // halo ring
    }

    #[test]
    fn pins_off_the_active_part_are_skipped() {
        let mut s = empty();
        let mut p1 = SymbolPin::new("1", "A");
        p1.part_number = 1;
        let mut p2 = SymbolPin::new("2", "B");
        p2.part_number = 2;
        let mut p0 = SymbolPin::new("3", "C");
        p0.part_number = 0; // part-zero: always visible
        s.pins.push(p1);
        s.pins.push(p2);
        s.pins.push(p0);
        // Active part 1 → pin 1 (part 1) + pin 3 (part 0) = 2 stubs.
        let snap = build_symbol_snapshot(&s, &None, 1, &colors(), 10.0);
        assert_eq!(snap.junctions.len(), 2);
    }
}
