//! #2596 (DECISIONS 2026-11-27): switching a pattern through every kind and
//! back keeps its count, and a skipped instance and a pad override follow
//! their instance around the trip.

use signex_bake::array::{KindTarget, switch_pattern_kind};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::PadAttrOverride;

#[test]
fn a_pattern_goes_through_every_kind_and_comes_back_with_its_instances() {
    let centre = SketchEntityId::new();
    let start = ArrayKind::Linear {
        source: SketchEntityId::new(),
        count_expr: "6".into(),
        dx_expr: "1.27mm".into(),
        dy_expr: "0mm".into(),
        depopulation: Some(GridDepopulation {
            mask_expr: String::new(),
            suppressed_instances: vec![(4, 0)],
        }),
    };
    let array = Array {
        id: ArrayId::new(),
        kind: start.clone(),
        numbering: NumberingScheme::default(),
    };
    let id = array.id;
    let mut sketch = SketchData {
        arrays: vec![array],
        ..SketchData::default()
    };
    let net = PadAttrOverride {
        net: Some("GND".into()),
        ..PadAttrOverride::default()
    };
    sketch.set_override(id, 2, 0, net.clone());

    for target in [
        KindTarget::Grid,
        KindTarget::Polar { centre },
        KindTarget::Linear,
    ] {
        let dropped = switch_pattern_kind(&mut sketch, id, target).expect("a switch");
        assert!(dropped.suppressed_dropped.is_empty(), "{target:?}");
        assert!(dropped.overrides_dropped.is_empty(), "{target:?}");
        let kind = &sketch.arrays[0].kind;
        assert!(kind.is_instance_suppressed(4, 0), "{target:?}");
        assert_eq!(sketch.override_for(id, 2, 0), Some(&net), "{target:?}");
    }
    let ArrayKind::Linear { count_expr, .. } = &sketch.arrays[0].kind else {
        panic!("back to linear");
    };
    assert_eq!(count_expr, "6");
}
