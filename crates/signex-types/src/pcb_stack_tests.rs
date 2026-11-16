//! #2401: the derived stack, copper naming by position and per-layer sizes.

use super::*;
use crate::pcb::Point;
use crate::pcb_layer::InnerCopper;

fn def(name: &str, layer_type: &str) -> LayerDef {
    LayerDef {
        id: 0,
        name: name.into(),
        layer_type: layer_type.into(),
        display_name: String::new(),
    }
}

#[test]
fn a_board_without_layers_derives_two_copper_layers_summing_to_its_thickness() {
    let stack = LayerStack::derived(&[], 1.6);
    let copper: Vec<&str> = stack.copper().map(|l| l.layer.as_str()).collect();
    assert_eq!(copper, ["TopCopper", "BottomCopper"]);
    assert!((stack.total_thickness_mm() - 1.6).abs() < 1e-9);
    assert_eq!(
        stack.layers.first().map(|l| l.kind),
        Some(StackLayerKind::Overlay)
    );
    assert_eq!(
        stack.layers.last().map(|l| l.kind),
        Some(StackLayerKind::Overlay)
    );
}

#[test]
fn the_derived_stack_follows_the_layer_list_and_marks_planes() {
    let layers = [
        def("TopCopper", "signal"),
        def("Inner1Copper", "plane"),
        def("Inner2Copper", "signal"),
        def("BottomCopper", "signal"),
        def("TopSilkscreen", ""),
    ];
    let stack = LayerStack::derived(&layers, 1.6);
    let copper: Vec<(&str, StackLayerKind)> =
        stack.copper().map(|l| (l.layer.as_str(), l.kind)).collect();
    assert_eq!(
        copper,
        [
            ("TopCopper", StackLayerKind::Signal),
            ("Inner1Copper", StackLayerKind::Plane),
            ("Inner2Copper", StackLayerKind::Signal),
            ("BottomCopper", StackLayerKind::Signal),
        ]
    );
    let dielectrics = stack
        .layers
        .iter()
        .filter(|l| l.kind == StackLayerKind::Dielectric)
        .count();
    assert_eq!(dielectrics, 3, "one dielectric between each copper pair");
    assert!((stack.total_thickness_mm() - 1.6).abs() < 1e-9);
}

#[test]
fn copper_is_named_by_position_top_inner_bottom() {
    assert_eq!(copper_at(0, 4), Some(CopperLayer::Top));
    assert_eq!(
        copper_at(2, 4),
        Some(CopperLayer::Inner(InnerCopper::new(2).unwrap()))
    );
    assert_eq!(copper_at(3, 4), Some(CopperLayer::Bottom));
    assert_eq!(copper_at(0, 1), None, "a board needs two copper layers");
    assert_eq!(copper_at(0, MAX_COPPER_LAYERS + 1), None);
}

#[test]
fn top_middle_bottom_sizes_read_per_layer() {
    let sizes = LayerSizes::TopMiddleBottom {
        top: 0.6,
        middle: 0.45,
        bottom: 0.6,
    };
    assert_eq!(sizes.on_token("TopCopper"), Some(0.6));
    assert_eq!(sizes.on_token("Inner3Copper"), Some(0.45));
    assert_eq!(sizes.on_token("BottomCopper"), Some(0.6));
    assert_eq!(sizes.on_token("TopSilkscreen"), None);
}

#[test]
fn a_via_with_a_smaller_inner_size_keeps_its_own_size_elsewhere() {
    let via: crate::pcb::Via = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "position": {"x": 0.0, "y": 0.0},
        "diameter": 0.6,
        "drill": 0.3,
        "layer_sizes": {"mode": "full", "layers": [
            {"layer": "Inner1Copper", "size": 0.45}
        ]}
    }))
    .unwrap();
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    assert_eq!(via.diameter_on(inner), 0.45);
    assert_eq!(via.diameter_on(CopperLayer::Top), 0.6);
    assert_eq!(via.min_diameter(), 0.45);
}

#[test]
fn a_pad_size_reads_per_layer() {
    let sizes = LayerSizes::Full {
        layers: vec![LayerSize::new(CopperLayer::Bottom, Point::new(0.8, 0.5))],
    };
    assert_eq!(sizes.on(CopperLayer::Bottom), Some(Point::new(0.8, 0.5)));
    assert_eq!(sizes.on(CopperLayer::Top), None);
}

#[test]
fn a_row_with_a_negative_or_non_finite_number_is_not_valid() {
    let mut row = StackLayer::new(StackLayerKind::Dielectric);
    assert!(row.numbers_valid());
    row.thickness_mm = -0.1;
    assert!(!row.numbers_valid());
    row.thickness_mm = f64::NAN;
    assert!(!row.numbers_valid());
}
