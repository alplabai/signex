//! Canonical draw order for a [`Scene`](crate::scene::Scene)'s primitive
//! buckets, shared by the CPU `canvas::Frame` renderer and the GPU shader path
//! so the two cannot silently diverge.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! Every draw path walks a const from here, so reordering one means editing
//! this file — and the parity tests below fail until the sides agree:
//! `scene_shader::ScenePrimitive::draw` walks [`GPU_SCENE_DRAW_ORDER`],
//! `pcb_canvas::draw_scene` walks [`CPU_PCB_DRAW_ORDER`], and
//! `renderer_scene_canvas::draw_scene_with_world_to_screen` walks
//! [`CPU_SCHEMATIC_DRAW_ORDER`].
//!
//! That last one is new in #645. Until then the schematic / Symbol Editor
//! replay hardcoded its own sequence, so the guarantee this module advertises
//! covered the PCB and quietly did not cover the surface that #199 is about
//! to move onto the GPU. Issue #1 (dashed lines render solid on GPU) and issue #4
//! (an overlay polygon drawing under base content) are fixed — `line.wgsl`
//! now honours the dash style bit and `scene_shader::ScenePrimitive::draw`
//! composites overlays in a dedicated pass after every base bucket, matching
//! the CPU's overlay pass; `overlays_composite_above_base_buckets` below pins
//! that parity.
//!
//! The schematic's BASE bucket order used to be the one deliberate
//! divergence: the GPU drew Polygons before Lines/Circles, the schematic CPU
//! replay drew them after, so a symbol body fill covered the wire beneath it
//! on one path and sat under it on the other. That was a visual-authority
//! call, and it was made in #645 for the GPU order — fills at the back,
//! strokes over them, text on top — so the schematic replay now draws its
//! base buckets in exactly [`GPU_SCENE_DRAW_ORDER`].
//!
//! The PCB had the same divergence — its CPU canvas drew zone fills and pads
//! over the traces running into them — and #653 settled it the same way, so
//! [`CPU_PCB_DRAW_ORDER`]'s base pass is now the GPU order too, minus the arc
//! and text buckets the PCB never emits. Every base pass on every path now
//! puts fills at the back, with one deliberate exception: pads.
//!
//! Pads are the exception because the maintainer's call for the PCB is that a
//! pad renders ABOVE the traces and vias that land on it, while zone pours and
//! rule areas stay beneath them. So pads go to their own
//! [`SceneBucket::TopPolygons`] bucket, drawn after Circles and before Texts on
//! every path; the PCB base z-order is zones/rule areas, traces, vias, pads,
//! then overlays.

/// A drawable bucket of a [`Scene`](crate::scene::Scene). Every variant maps
/// 1:1 to a `Vec` field on the scene; the order of a slice of these is a draw
/// (z) order, back to front.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SceneBucket {
    Lines,
    Circles,
    Arcs,
    Polygons,
    /// Polygons composited above the stroke buckets; PCB pads are its
    /// producer. Maps to `Scene::top_polygons`.
    TopPolygons,
    Texts,
    OverlayLines,
    OverlayCircles,
    OverlayPolygons,
    ErcMarkerLines,
    ErcMarkerCircles,
    ErcMarkerPolygons,
}

/// Draw order of the GPU scene shader (`scene_shader::ScenePrimitive::draw`):
/// fills, then strokes, then the top polygons (PCB pads), then text on top. Overlay and ERC buckets never
/// appear here — `ScenePrimitive::draw` composites overlay geometry in its
/// own dedicated pass strictly AFTER this order (so it always renders on
/// top, matching the CPU's separate overlay pass below), and ERC markers are
/// schematic-only.
pub const GPU_SCENE_DRAW_ORDER: &[SceneBucket] = &[
    SceneBucket::Polygons,
    SceneBucket::Lines,
    SceneBucket::Arcs,
    SceneBucket::Circles,
    SceneBucket::TopPolygons,
    SceneBucket::Texts,
];

/// Draw order of the schematic / Symbol Editor CPU renderer
/// (`renderer_scene_canvas::draw_scene_with_world_to_screen`): base geometry,
/// then overlays, then ERC markers last and therefore topmost.
///
/// The only order here that names every [`SceneBucket`] — this replay is the
/// one path that draws all twelve.
///
/// Its six base buckets are [`GPU_SCENE_DRAW_ORDER`], in that order: fills
/// first, so a symbol body fill or a label-marker flag sits *under* the wires
/// and pins that run into it. Until #645 this replay drew Polygons fourth —
/// after Lines, Circles and Arcs — and a body fill hid the wire beneath it,
/// the opposite of what the GPU would show once #199 mounts the schematic.
/// [`SceneBucket::TopPolygons`] holds its place in that order but is always
/// empty here — only the PCB emits it.
///
/// Added by #645. Before that this sequence was hardcoded in the replay with
/// no const and no test, so the module doc above held only for the PCB: the
/// surface about to be migrated to the GPU was invisible to the very
/// mechanism meant to stop it drifting.
///
/// The three ERC buckets are drawn here but are **empty in production**:
/// `SchematicRenderer::build_scene` fills them only under
/// `DirtyFlags::OVERLAY`, and the one production caller that passes that flag
/// (`schematic_runtime::overlay::draw_erc_markers`) routes its markers through
/// `OverlayInputs` instead. They stay in the order because the replay does
/// walk them, and a future producer would otherwise land them at the wrong
/// depth.
pub const CPU_SCHEMATIC_DRAW_ORDER: &[SceneBucket] = &[
    SceneBucket::Polygons,
    SceneBucket::Lines,
    SceneBucket::Arcs,
    SceneBucket::Circles,
    SceneBucket::TopPolygons,
    SceneBucket::Texts,
    SceneBucket::OverlayLines,
    SceneBucket::OverlayCircles,
    SceneBucket::OverlayPolygons,
    SceneBucket::ErcMarkerLines,
    SceneBucket::ErcMarkerCircles,
    SceneBucket::ErcMarkerPolygons,
];

/// Draw order of the PCB CPU canvas renderer (`pcb_canvas::draw_scene`): main
/// geometry, then a second overlay pass. No arc or text buckets — the PCB scene
/// never emits them on the CPU path.
///
/// Back to front: zone pours and rule areas (Polygons), traces (Lines), vias
/// (Circles), then pads (TopPolygons), as on the GPU. A zone sits under the
/// traces that run into it; a pad sits on top of them. Until #653 this drew
/// Polygons last, so the same board stacked the other way round depending on
/// the GPU-render preference.
pub const CPU_PCB_DRAW_ORDER: &[SceneBucket] = &[
    SceneBucket::Polygons,
    SceneBucket::Lines,
    SceneBucket::Circles,
    SceneBucket::TopPolygons,
    SceneBucket::OverlayLines,
    SceneBucket::OverlayCircles,
    SceneBucket::OverlayPolygons,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::circle::Circle;
    use crate::primitive::line::LineSegment;
    use crate::primitive::polygon::GpuPolygon;

    fn index_of(order: &[SceneBucket], bucket: SceneBucket) -> Option<usize> {
        order.iter().position(|b| *b == bucket)
    }

    #[test]
    fn gpu_scene_draw_order_is_fills_then_strokes_then_text() {
        let expected: &[SceneBucket] = &[
            SceneBucket::Polygons,
            SceneBucket::Lines,
            SceneBucket::Arcs,
            SceneBucket::Circles,
            SceneBucket::TopPolygons,
            SceneBucket::Texts,
        ];
        assert_eq!(GPU_SCENE_DRAW_ORDER, expected);
    }

    #[test]
    fn cpu_pcb_draw_order_is_main_geometry_then_overlays() {
        let expected: &[SceneBucket] = &[
            SceneBucket::Polygons,
            SceneBucket::Lines,
            SceneBucket::Circles,
            SceneBucket::TopPolygons,
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
        ];
        assert_eq!(CPU_PCB_DRAW_ORDER, expected);
    }

    /// #4 (fixed): both paths composite overlay geometry strictly AFTER every
    /// base bucket, so a DRC marker / selection highlight / active-layer zone
    /// highlight always renders on top of ordinary geometry regardless of
    /// backend. On GPU this holds structurally — no overlay bucket appears in
    /// [`GPU_SCENE_DRAW_ORDER`] at all; `scene_shader::ScenePrimitive::draw`
    /// composites the overlay trio in its own pass strictly after this order
    /// (see the module doc and `pcb_canvas::gpu_scene_keeps_overlays_out_of_the_base_buckets`).
    /// On CPU, [`CPU_PCB_DRAW_ORDER`] lists every overlay bucket after every
    /// base bucket. Before the fix, `pcb_canvas::gpu_scene` folded overlay
    /// geometry into the base buckets upstream of this order, so a GPU
    /// overlay polygon landed in the first-drawn base `Polygons` bucket
    /// instead of on top — this test pins that it cannot regress.
    #[test]
    fn overlays_composite_above_base_buckets() {
        let base_buckets = [
            SceneBucket::Lines,
            SceneBucket::Circles,
            SceneBucket::Polygons,
            SceneBucket::TopPolygons,
        ];
        let overlay_buckets = [
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
        ];

        for bucket in overlay_buckets {
            assert!(
                !GPU_SCENE_DRAW_ORDER.contains(&bucket),
                "GPU base pass must never carry an overlay bucket — overlays \
                 composite in their own later pass instead"
            );
        }

        let max_base_idx = base_buckets
            .iter()
            .map(|b| index_of(CPU_PCB_DRAW_ORDER, *b).unwrap())
            .max()
            .unwrap();
        let min_overlay_idx = overlay_buckets
            .iter()
            .map(|b| index_of(CPU_PCB_DRAW_ORDER, *b).unwrap())
            .min()
            .unwrap();
        assert!(
            min_overlay_idx > max_base_idx,
            "CPU must draw every overlay bucket after every base bucket"
        );
    }

    #[test]
    fn cpu_schematic_draw_order_is_base_then_overlays_then_erc() {
        let expected: &[SceneBucket] = &[
            SceneBucket::Polygons,
            SceneBucket::Lines,
            SceneBucket::Arcs,
            SceneBucket::Circles,
            SceneBucket::TopPolygons,
            SceneBucket::Texts,
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
            SceneBucket::ErcMarkerLines,
            SceneBucket::ErcMarkerCircles,
            SceneBucket::ErcMarkerPolygons,
        ];
        assert_eq!(CPU_SCHEMATIC_DRAW_ORDER, expected);
    }

    /// The schematic replay is the only path that draws every bucket, so its
    /// order is also the completeness check: add a `SceneBucket` variant and
    /// this fails until someone decides where it belongs.
    #[test]
    fn the_schematic_order_names_every_bucket_exactly_once() {
        for bucket in [
            SceneBucket::Lines,
            SceneBucket::Circles,
            SceneBucket::Arcs,
            SceneBucket::Polygons,
            SceneBucket::TopPolygons,
            SceneBucket::Texts,
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
            SceneBucket::ErcMarkerLines,
            SceneBucket::ErcMarkerCircles,
            SceneBucket::ErcMarkerPolygons,
        ] {
            assert_eq!(
                CPU_SCHEMATIC_DRAW_ORDER
                    .iter()
                    .filter(|b| **b == bucket)
                    .count(),
                1,
                "{bucket:?} must appear exactly once in CPU_SCHEMATIC_DRAW_ORDER"
            );
        }
    }

    /// Overlays and ERC markers are presentation: they must land on top of
    /// every base bucket on the schematic replay, exactly as the PCB rule
    /// above requires for its own overlays.
    #[test]
    fn schematic_overlays_and_erc_markers_composite_above_base_buckets() {
        let base = [
            SceneBucket::Lines,
            SceneBucket::Circles,
            SceneBucket::Arcs,
            SceneBucket::Polygons,
            SceneBucket::TopPolygons,
            SceneBucket::Texts,
        ];
        let above = [
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
            SceneBucket::ErcMarkerLines,
            SceneBucket::ErcMarkerCircles,
            SceneBucket::ErcMarkerPolygons,
        ];

        let max_base = base
            .iter()
            .map(|b| index_of(CPU_SCHEMATIC_DRAW_ORDER, *b).unwrap())
            .max()
            .unwrap();
        let min_above = above
            .iter()
            .map(|b| index_of(CPU_SCHEMATIC_DRAW_ORDER, *b).unwrap())
            .min()
            .unwrap();

        assert!(
            min_above > max_base,
            "every overlay and ERC bucket must draw after every base bucket"
        );
    }

    /// The schematic replay's base pass IS the GPU order — the parity #199's
    /// mount depends on. Symbol body fills are polygons and wires are lines;
    /// until #645 the two paths stacked them opposite ways, and this test
    /// asserted that divergence. The visual call went to the GPU order, so it
    /// now asserts the opposite: change either side alone and it fails.
    #[test]
    fn schematic_cpu_base_pass_matches_the_gpu_order() {
        assert_eq!(
            &CPU_SCHEMATIC_DRAW_ORDER[..GPU_SCENE_DRAW_ORDER.len()],
            GPU_SCENE_DRAW_ORDER
        );
    }

    /// The PCB CPU canvas's base pass is the GPU order with the buckets the
    /// PCB never emits (arcs, text) left out. Until #653 it drew fills after
    /// strokes and this test pinned that divergence; now it pins parity, so
    /// reordering either side alone fails.
    #[test]
    fn pcb_cpu_base_pass_matches_the_gpu_order() {
        let gpu_without_unemitted: Vec<SceneBucket> = GPU_SCENE_DRAW_ORDER
            .iter()
            .copied()
            .filter(|bucket| !matches!(bucket, SceneBucket::Arcs | SceneBucket::Texts))
            .collect();
        let cpu_base: Vec<SceneBucket> = CPU_PCB_DRAW_ORDER
            .iter()
            .copied()
            .take_while(|bucket| {
                !matches!(
                    bucket,
                    SceneBucket::OverlayLines
                        | SceneBucket::OverlayCircles
                        | SceneBucket::OverlayPolygons
                )
            })
            .collect();
        assert_eq!(cpu_base, gpu_without_unemitted);
    }

    /// Pads are the one polygon bucket that draws above the strokes: after
    /// Circles (so over traces and vias) and before Texts, on every path, with
    /// the zone/rule-area `Polygons` bucket still at the very back.
    #[test]
    fn top_polygons_draw_above_strokes_and_polygons_below() {
        for order in [
            GPU_SCENE_DRAW_ORDER,
            CPU_PCB_DRAW_ORDER,
            CPU_SCHEMATIC_DRAW_ORDER,
        ] {
            let top = index_of(order, SceneBucket::TopPolygons).unwrap();
            for under in [
                SceneBucket::Polygons,
                SceneBucket::Lines,
                SceneBucket::Circles,
            ] {
                assert!(
                    index_of(order, under).unwrap() < top,
                    "{under:?} must draw under pads"
                );
            }
            if let Some(text) = index_of(order, SceneBucket::Texts) {
                assert!(top < text, "text stays above pads");
            }
        }
    }

    /// The GPU path composites arc and text buckets the PCB CPU path never
    /// emits. Harmless today (PCB scenes carry neither) but a real parity axis:
    /// if the PCB scene ever grows arcs or text, the CPU renderer must gain the
    /// buckets too.
    #[test]
    fn gpu_draws_buckets_the_pcb_cpu_path_omits() {
        for bucket in [SceneBucket::Arcs, SceneBucket::Texts] {
            assert!(GPU_SCENE_DRAW_ORDER.contains(&bucket));
            assert!(!CPU_PCB_DRAW_ORDER.contains(&bucket));
        }
    }

    /// Neither draw order composites overlay or ERC buckets through the
    /// shader's base pass: overlay geometry gets its own dedicated pass
    /// strictly after `GPU_SCENE_DRAW_ORDER` (see
    /// `scene_shader::ScenePrimitive::draw`), and ERC markers are
    /// schematic-only.
    #[test]
    fn scene_shader_composites_no_overlay_or_erc_buckets() {
        for bucket in [
            SceneBucket::OverlayLines,
            SceneBucket::OverlayCircles,
            SceneBucket::OverlayPolygons,
            SceneBucket::ErcMarkerLines,
            SceneBucket::ErcMarkerCircles,
            SceneBucket::ErcMarkerPolygons,
        ] {
            assert!(!GPU_SCENE_DRAW_ORDER.contains(&bucket));
        }
    }

    #[test]
    fn line_dash_predicate_reads_the_low_style_bit() {
        let mut line = LineSegment {
            p0: [0.0, 0.0],
            p1: [1.0, 0.0],
            width: 0.2,
            color: [1.0, 1.0, 1.0, 1.0],
            style: LineSegment::STYLE_DASHED,
            _pad: 0,
        };
        assert!(line.is_dashed());
        line.style = 0;
        assert!(!line.is_dashed());
    }

    // #1 (fixed): `is_dashed()` above is the Rust-side contract; the actual
    // GPU-visible fix — `line.wgsl` forwarding `style` and discarding the
    // gap portions of a dashed segment — is pixel-verified by
    // `debug_pass::tests::line_wgsl_actually_renders_a_dashed_pattern`
    // (a Rust-only assertion here can't see shader text, so it belongs
    // there, not as a second copy of the predicate check above).

    #[test]
    fn circle_fill_predicate_splits_on_stroke_width() {
        let filled = Circle {
            center: [0.0, 0.0],
            radius: 1.0,
            stroke_width: 0.0,
            color: [1.0; 4],
        };
        let ring = Circle {
            center: [0.0, 0.0],
            radius: 1.0,
            stroke_width: 0.2,
            color: [1.0; 4],
        };
        assert!(filled.is_filled());
        assert!(!ring.is_filled());
    }

    #[test]
    fn polygon_stroke_predicate_needs_only_a_colour() {
        let base = GpuPolygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            fill_color: [0.2, 0.2, 0.8, 1.0],
            stroke_color: None,
            stroke_width: 0.5,
        };
        assert!(!base.is_stroked(), "a width with no colour draws nothing");

        let coloured_zero_width = GpuPolygon {
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.0,
            ..base.clone()
        };
        // A hairline on both paths since #645 — the CPU replay always stroked
        // it at its floor; the GPU used to drop it.
        assert!(coloured_zero_width.is_stroked());

        let stroked = GpuPolygon {
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.1,
            ..base
        };
        assert!(stroked.is_stroked());
    }
}
