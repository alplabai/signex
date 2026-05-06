//! Integration tests for the v0.13 sketch → footprint pad bake.
//!
//! Phase 7 Task 7.1 + 7.2 of the SKETCH_MODE_v0.13_PLAN. Each test
//! constructs a small `SketchData` inline, runs the solver to produce
//! a `FullSolveOutput`, then bakes via `signex_bake::bake_pads`
//! / `bake_arrays` and asserts the resulting `LibPad` set.
//!
//! Cleanroom: no third-party constraint-solver, footprint-generator,
//! or numerical-library source consulted.

use std::collections::HashMap;

use signex_bake::{bake_arrays, bake_pads};
use signex_library::primitive::footprint::{
    LayerId, Pad as LibPad, PadKind as LibPadKind, PadShape as LibPadShape,
};
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{
    ChamferedCorners, DrillSpec, PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern,
};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

// ─────────────────────────────────────────────────────────────────────
// Inline sketch builder — no shared `tests/common/` for this crate.
// ─────────────────────────────────────────────────────────────────────

struct Sketch {
    data: SketchData,
    plane: PlaneId,
}

impl Sketch {
    fn new() -> Self {
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let mut data = SketchData::default();
        let plane_id = plane.id;
        data.planes.push(plane);
        Self {
            data,
            plane: plane_id,
        }
    }

    /// Add a Point and return its ID. The `pad` attr can be attached
    /// afterwards by writing into `data.entities[idx].pad`.
    fn add_point(&mut self, x: f64, y: f64) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Point { x, y }));
        id
    }

    /// Mutate the most-recently-added entity (or the entity with `id`)
    /// to attach the given `PadAttr`.
    fn attach_pad(&mut self, id: SketchEntityId, pad: PadAttr) {
        let e = self
            .data
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .expect("attach_pad: entity not found");
        e.pad = Some(pad);
    }

    /// Set the construction flag on the entity with `id`.
    fn set_construction(&mut self, id: SketchEntityId, value: bool) {
        let e = self
            .data
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .expect("set_construction: entity not found");
        e.construction = value;
    }
}

// ─────────────────────────────────────────────────────────────────────
// Helpers for building common PadAttr fixtures.
// ─────────────────────────────────────────────────────────────────────

fn smd_rect_pad(number: &str, w: &str, h: &str) -> PadAttr {
    PadAttr {
        number: number.into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: w.into(),
        size_y_expr: h.into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: None,
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

fn solve(sketch: &SketchData) -> signex_sketch::solver::FullSolveOutput {
    let solver = Solver::default();
    solver
        .solve(sketch, &ResolvedParams::new())
        .expect("solve must succeed for these test fixtures")
}

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn has_layer(pad: &LibPad, name: &str) -> bool {
    pad.layers.iter().any(|l| l.as_str() == name)
}

// ─────────────────────────────────────────────────────────────────────
// Test 1 — basic SMD rect pad on the top side.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_smd_rect_pad() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    s.attach_pad(p, smd_rect_pad("1", "1.0mm", "0.5mm"));

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake_pads ok");

    assert_eq!(out.len(), 1);
    let pad = &out[0];
    assert_eq!(pad.kind, LibPadKind::Smd);
    assert_eq!(pad.shape, LibPadShape::Rect);
    assert!(approx_eq(pad.size[0], 1.0, 1e-9));
    assert!(approx_eq(pad.size[1], 0.5, 1e-9));
    assert!(approx_eq(pad.position[0], 0.0, 1e-9));
    assert!(approx_eq(pad.position[1], 0.0, 1e-9));
    assert!(has_layer(pad, "Top Layer"));
    assert!(has_layer(pad, "Top Solder"));
    assert!(has_layer(pad, "Top Paste"));
    assert_eq!(pad.layers.len(), 3);
    assert!(pad.drill.is_none());
}

// ─────────────────────────────────────────────────────────────────────
// Test 2 — Round pad with parametric size.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_round_pad_with_param_size() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("1", "= pad_w", "= pad_w");
    pad.shape = PadShape::Round;
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut params = HashMap::new();
    params.insert("pad_w".to_string(), 0.6);

    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &params, &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].shape, LibPadShape::Round);
    assert!(approx_eq(out[0].size[0], 0.6, 1e-9));
    assert!(approx_eq(out[0].size[1], 0.6, 1e-9));
}

// ─────────────────────────────────────────────────────────────────────
// Test 3 — THT pad with round drill, side=All.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_tht_pad_with_drill() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("1", "1.5mm", "1.5mm");
    pad.kind = PadKind::Tht;
    pad.side = PadSide::All;
    pad.shape = PadShape::Round;
    pad.drill = Some(DrillSpec {
        diameter_expr: "0.8mm".into(),
        slot_length_expr: None,
        plated: true,
    });
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    let pad = &out[0];
    assert_eq!(pad.kind, LibPadKind::Tht);
    let drill = pad.drill.as_ref().expect("drill present");
    assert!(approx_eq(drill.diameter, 0.8, 1e-9));
    assert!(drill.slot_length.is_none());

    // THT layers: F+B copper + F+B mask, no paste.
    assert!(has_layer(pad, "Top Layer"));
    assert!(has_layer(pad, "Bottom Layer"));
    assert!(has_layer(pad, "Top Solder"));
    assert!(has_layer(pad, "Bottom Solder"));
    assert!(!has_layer(pad, "Top Paste"));
    assert!(!has_layer(pad, "Bottom Paste"));
    assert_eq!(pad.layers.len(), 4);
}

// ─────────────────────────────────────────────────────────────────────
// Test 4 — NPT mounting hole — mask only, no copper.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_npt_hole_pad() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("MOUNT", "3.5mm", "3.5mm");
    pad.kind = PadKind::NptHole;
    pad.shape = PadShape::Round;
    pad.drill = Some(DrillSpec {
        diameter_expr: "3.2mm".into(),
        slot_length_expr: None,
        plated: false,
    });
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    let pad = &out[0];
    assert_eq!(pad.kind, LibPadKind::NptHole);
    assert!(!has_layer(pad, "Top Layer"));
    assert!(!has_layer(pad, "Bottom Layer"));
    assert!(has_layer(pad, "Top Solder"));
    assert!(has_layer(pad, "Bottom Solder"));
    assert_eq!(pad.layers.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────
// Test 5 — Fiducial without mask_margin defaults to 1.0mm.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_fiducial_default_mask_margin() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("FID1", "1.0mm", "1.0mm");
    pad.kind = PadKind::Fiducial;
    pad.shape = PadShape::Round;
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    let pad = &out[0];
    assert_eq!(pad.kind, LibPadKind::Fiducial, "v0.14: native Fiducial");
    assert_eq!(pad.shape, LibPadShape::Round);
    assert_eq!(pad.solder_mask_margin, Some(1.0));
    assert!(pad.paste_margin.is_none());
    assert!(pad.drill.is_none());
    // Top Fiducial: Top Layer + Top Solder, no paste, no bottom.
    assert!(has_layer(pad, "Top Layer"));
    assert!(has_layer(pad, "Top Solder"));
    assert!(!has_layer(pad, "Top Paste"));
    assert!(!has_layer(pad, "Bottom Layer"));
    assert_eq!(pad.layers.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────
// Test 6 — Castellated bakes as a native LibPadKind::Castellated (v0.14).
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_castellated_native() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("E1", "1.5mm", "1.0mm");
    pad.kind = PadKind::Castellated;
    pad.side = PadSide::All;
    pad.shape = PadShape::RoundRect {
        radius_ratio_expr: "0.25".into(),
    };
    pad.drill = Some(DrillSpec {
        diameter_expr: "0.6mm".into(),
        slot_length_expr: None,
        plated: true,
    });
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    let pad = &out[0];
    assert_eq!(
        pad.kind,
        LibPadKind::Castellated,
        "v0.14: native Castellated"
    );
    let drill = pad.drill.as_ref().expect("drill present");
    assert!(approx_eq(drill.diameter, 0.6, 1e-9));
    // Castellated reuses the THT layer pattern (no paste).
    assert!(has_layer(pad, "Top Layer"));
    assert!(has_layer(pad, "Bottom Layer"));
    assert!(has_layer(pad, "Top Solder"));
    assert!(has_layer(pad, "Bottom Solder"));
    assert!(!has_layer(pad, "Top Paste"));
    // No warning anymore — variant is native in v0.14.
    assert!(
        warnings.iter().all(|w| !w.contains("Castellated")),
        "v0.14 should not warn on Castellated; got {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Test 7 — Chamfered shape bakes natively as LibPadShape::Chamfered (v0.14).
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_chamfered_native() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("1", "1.0mm", "0.5mm");
    pad.shape = PadShape::Chamfered {
        chamfer_ratio_expr: "0.2".into(),
        corners: ChamferedCorners::ALL,
    };
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    match out[0].shape {
        LibPadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            assert!(approx_eq(chamfer_ratio, 0.2, 1e-9));
            assert!(
                corners.top_left
                    && corners.top_right
                    && corners.bottom_left
                    && corners.bottom_right
            );
        }
        ref other => panic!("expected Chamfered, got {other:?}"),
    }
    // No warning anymore — variant is native in v0.14.
    assert!(
        warnings.iter().all(|w| !w.contains("Chamfered")),
        "v0.14 should not warn on Chamfered; got {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Test 8 — Paste Grid pattern warns and falls back to Single.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_paste_grid_warns_and_falls_back_to_single() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    let mut pad = smd_rect_pad("EP", "3.0mm", "3.0mm");
    pad.paste_apertures = PasteAperturePattern::Grid {
        nx_expr: "3".into(),
        ny_expr: "3".into(),
        coverage_expr: "0.6".into(),
    };
    s.attach_pad(p, pad);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    // One pad, single aperture — exactly as if PasteAperturePattern::Single.
    assert_eq!(out.len(), 1);
    assert!(has_layer(&out[0], "Top Paste"));
    assert!(
        warnings.iter().any(|w| w.contains("Grid")),
        "expected Grid warning, got {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Test 9 — construction=true entities are skipped.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_construction_entities_skipped() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    s.attach_pad(p, smd_rect_pad("1", "1.0mm", "1.0mm"));
    s.set_construction(p, true);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert!(out.is_empty(), "construction entity must not bake");
    // No warning needed — construction skip is silent.
}

// ─────────────────────────────────────────────────────────────────────
// Test 9b — construction entities with closed-profile attrs do NOT
// emit "v0.14 deferred" warnings. Construction entities are scaffold
// and never produce baked geometry, so warning about their bake-attrs
// would be misleading noise.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_construction_entity_with_silk_attr_emits_no_warning() {
    use signex_sketch::attr::SilkAttr;
    use signex_types::layer::SignexLayer;

    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    // Tag with a SilkAttr (would normally trigger a "v0.14 feature"
    // warning) AND mark as construction. The construction flag should
    // win — no warning should appear.
    {
        let e = s.data.entities.iter_mut().find(|e| e.id == p).unwrap();
        e.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
    }
    s.set_construction(p, true);

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert!(out.is_empty(), "construction entity must not bake to a pad");
    assert!(
        warnings.is_empty(),
        "construction entity must not emit v0.14-deferred warnings; got {warnings:?}"
    );
}

#[test]
fn bake_non_construction_entity_with_silk_attr_no_longer_warns_in_v014() {
    // v0.14: silk bake moved to crate::silk; pad.rs no longer warns
    // about SilkAttr — the dispatcher invokes bake_silk separately.
    use signex_sketch::attr::SilkAttr;
    use signex_types::layer::SignexLayer;

    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    s.attach_pad(p, smd_rect_pad("1", "1.0mm", "1.0mm"));
    {
        let e = s.data.entities.iter_mut().find(|e| e.id == p).unwrap();
        e.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
    }

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1, "pad still bakes");
    assert!(
        warnings.iter().all(|w| !w.contains("SilkAttr")),
        "v0.14: pad.rs must not warn on SilkAttr (silk bake lives in crate::silk now); got {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Test 10 — LinearArray with count=3, dx=1mm, dy=0 → 3 pads on x axis.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_linear_array_3_pads_along_x() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    s.attach_pad(p, smd_rect_pad("0", "1.0mm", "0.5mm"));

    s.data.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: p,
            count_expr: "3".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
        },
    });

    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_arrays(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 3);
    for (i, pad) in out.iter().enumerate() {
        assert!(approx_eq(pad.position[0], i as f64, 1e-9));
        assert!(approx_eq(pad.position[1], 0.0, 1e-9));
    }
    // Numbers: 1, 2, 3.
    assert_eq!(out[0].number, "1");
    assert_eq!(out[1].number, "2");
    assert_eq!(out[2].number, "3");
}

// ─────────────────────────────────────────────────────────────────────
// Test 11 — Custom::SketchProfile native bake (v0.14.1).
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_custom_sketch_profile_native_v0141() {
    use signex_sketch::attr::{CustomPadShape, PadShape};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::Plane;

    let mut s = Sketch::new();
    let plane = s.plane;
    let pad_pt = s.add_point(5.0, 5.0);

    // Build a 1×1 mm rectangle (4 lines + 4 corner Points) located at
    // (5, 5) → (6, 6) in footprint mm.
    let p1 = SketchEntityId::new();
    let p2 = SketchEntityId::new();
    let p3 = SketchEntityId::new();
    let p4 = SketchEntityId::new();
    s.data
        .entities
        .push(Entity::new(p1, plane, EntityKind::Point { x: 5.0, y: 5.0 }));
    s.data
        .entities
        .push(Entity::new(p2, plane, EntityKind::Point { x: 6.0, y: 5.0 }));
    s.data
        .entities
        .push(Entity::new(p3, plane, EntityKind::Point { x: 6.0, y: 6.0 }));
    s.data
        .entities
        .push(Entity::new(p4, plane, EntityKind::Point { x: 5.0, y: 6.0 }));
    let l1 = SketchEntityId::new();
    let l2 = SketchEntityId::new();
    let l3 = SketchEntityId::new();
    let l4 = SketchEntityId::new();
    s.data.entities.push(Entity::new(
        l1,
        plane,
        EntityKind::Line { start: p1, end: p2 },
    ));
    s.data.entities.push(Entity::new(
        l2,
        plane,
        EntityKind::Line { start: p2, end: p3 },
    ));
    s.data.entities.push(Entity::new(
        l3,
        plane,
        EntityKind::Line { start: p3, end: p4 },
    ));
    s.data.entities.push(Entity::new(
        l4,
        plane,
        EntityKind::Line { start: p4, end: p1 },
    ));

    // Pad sits at the rectangle's lower-left corner (5, 5). Profile
    // baked relative to pad position: world (5,5)→(6,6) becomes
    // local (0,0)→(1,1).
    let mut pad = smd_rect_pad("1", "1.0mm", "1.0mm");
    pad.shape = PadShape::Custom(CustomPadShape::SketchProfile { source: vec![l1] });
    s.attach_pad(pad_pt, pad);
    // Need to suppress the pad-point's own contribution as a corner —
    // ensure pad_pt isn't picked up by the walker. Setting it as
    // construction would also drop it from the topology, so leave it
    // as a Point-kind entity (Points aren't edges so they don't enter
    // the adjacency).
    let _ = (l2, l3, l4);

    let _ = Plane {
        id: plane,
        kind: signex_sketch::plane::PlaneKind::BoardTop,
    };
    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");

    assert_eq!(out.len(), 1);
    match &out[0].shape {
        LibPadShape::Custom(poly) => {
            // 4 vertices, in pad-local mm.
            assert_eq!(poly.points.len(), 4);
            // All vertices in the unit square (relative to pad position).
            for [x, y] in &poly.points {
                assert!(*x >= -1e-6 && *x <= 1.0 + 1e-6, "x out of range: {x}");
                assert!(*y >= -1e-6 && *y <= 1.0 + 1e-6, "y out of range: {y}");
            }
        }
        other => panic!("expected Custom polygon, got {other:?}"),
    }
    assert!(
        warnings.iter().all(|w| !w.contains("falls back")),
        "v0.14.1 should not fall back; got {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Smoke: verify the LibPad structure round-trips through bake without
// dropping any layer info — guards against accidental dedup bugs.
// ─────────────────────────────────────────────────────────────────────

#[test]
fn bake_layers_use_altium_label_strings() {
    let mut s = Sketch::new();
    let p = s.add_point(0.0, 0.0);
    s.attach_pad(p, smd_rect_pad("1", "1.0mm", "0.5mm"));
    let solve = solve(&s.data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_pads(&s.data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake ok");
    let pad = &out[0];
    // Sanity check: at least one layer must be a known altium-label string.
    let want = ["Top Layer", "Top Solder", "Top Paste"];
    for w in want {
        assert!(
            pad.layers.iter().any(|l: &LayerId| l.as_str() == w),
            "expected layer '{w}' in {:?}",
            pad.layers
        );
    }
}
