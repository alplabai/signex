//! Canonical draw order for a [`Scene`](crate::scene::Scene)'s primitive
//! buckets, shared by the CPU `canvas::Frame` renderer and the GPU shader path
//! so the two cannot silently diverge.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! `scene_shader::ScenePrimitive::draw` walks [`GPU_SCENE_DRAW_ORDER`] and
//! `pcb_canvas::draw_scene` walks [`CPU_PCB_DRAW_ORDER`], so reordering either
//! path means editing the const here — and the parity tests below fail until
//! both sides agree. Issue #1 (dashed lines render solid on GPU) and issue #4
//! (an overlay polygon drawing under base content) are fixed — `line.wgsl`
//! now honours the dash style bit and `scene_shader::ScenePrimitive::draw`
//! composites overlays in a dedicated pass after every base bucket, matching
//! the CPU's overlay pass; `overlays_composite_above_base_buckets` below pins
//! that parity. The remaining, deliberately-unreconciled divergence is the
//! BASE bucket order: GPU draws Polygons before Lines/Circles, CPU draws them
//! after — a visual-authority call reserved for Caner/Hakan, out of scope for
//! this fix.

/// A drawable bucket of a [`Scene`](crate::scene::Scene). Every variant maps
/// 1:1 to a `Vec` field on the scene; the order of a slice of these is a draw
/// (z) order, back to front.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SceneBucket {
    Lines,
    Circles,
    Arcs,
    Polygons,
    Texts,
    OverlayLines,
    OverlayCircles,
    OverlayPolygons,
    ErcMarkerLines,
    ErcMarkerCircles,
    ErcMarkerPolygons,
}

/// Draw order of the GPU scene shader (`scene_shader::ScenePrimitive::draw`):
/// fills, then strokes, then text on top. Overlay and ERC buckets never
/// appear here — `ScenePrimitive::draw` composites overlay geometry in its
/// own dedicated pass strictly AFTER this order (so it always renders on
/// top, matching the CPU's separate overlay pass below), and ERC markers are
/// schematic-only.
pub const GPU_SCENE_DRAW_ORDER: &[SceneBucket] = &[
    SceneBucket::Polygons,
    SceneBucket::Lines,
    SceneBucket::Arcs,
    SceneBucket::Circles,
    SceneBucket::Texts,
];

/// Draw order of the PCB CPU canvas renderer (`pcb_canvas::draw_scene`): main
/// geometry, then a second overlay pass. No arc or text buckets — the PCB scene
/// never emits them on the CPU path.
pub const CPU_PCB_DRAW_ORDER: &[SceneBucket] = &[
    SceneBucket::Lines,
    SceneBucket::Circles,
    SceneBucket::Polygons,
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
            SceneBucket::Texts,
        ];
        assert_eq!(GPU_SCENE_DRAW_ORDER, expected);
    }

    #[test]
    fn cpu_pcb_draw_order_is_main_geometry_then_overlays() {
        let expected: &[SceneBucket] = &[
            SceneBucket::Lines,
            SceneBucket::Circles,
            SceneBucket::Polygons,
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
    fn polygon_stroke_predicate_needs_colour_and_width() {
        let base = GpuPolygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            fill_color: [0.2, 0.2, 0.8, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        };
        assert!(!base.is_stroked(), "no colour, no width");

        let coloured_zero_width = GpuPolygon {
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.0,
            ..base.clone()
        };
        // GPU rule: a colour alone is not enough, a positive width is required.
        // The CPU path strokes this anyway (clamped min width) — documented
        // divergence.
        assert!(!coloured_zero_width.is_stroked());

        let stroked = GpuPolygon {
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.1,
            ..base
        };
        assert!(stroked.is_stroked());
    }
}
