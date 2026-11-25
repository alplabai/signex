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

/// Mockup row 105: a derived stack mirrors; a thicker dielectric on one side
/// or a heavier copper layer does not.
#[test]
fn a_stack_is_symmetric_until_one_side_differs() {
    let inner = [
        def("TopCopper", "signal"),
        def("Inner1Copper", "signal"),
        def("Inner2Copper", "plane"),
        def("BottomCopper", "signal"),
    ];
    let stack = LayerStack::derived(&inner, 1.6);
    assert!(stack.is_symmetric(), "Signal and Plane may mirror");

    let mut thicker = stack.clone();
    let row = thicker
        .layers
        .iter()
        .position(|l| l.kind == StackLayerKind::Dielectric)
        .unwrap();
    thicker.layers[row].thickness_mm += 0.1;
    assert!(!thicker.is_symmetric());

    let mut heavier = stack;
    let row = heavier
        .layers
        .iter()
        .position(|l| l.kind.is_copper())
        .unwrap();
    heavier.layers[row].copper_weight_oz = 2.0;
    assert!(!heavier.is_symmetric());
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
    assert_eq!(
        via.min_diameter(&[CopperLayer::Top, inner, CopperLayer::Bottom]),
        0.45
    );
}

#[test]
fn a_top_middle_bottom_via_ignores_its_own_diameter() {
    let mut via: crate::pcb::Via = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "position": {"x": 0.0, "y": 0.0},
        "diameter": 0.45,
        "drill": 0.3,
        "layers": ["TopCopper", "BottomCopper"],
        "layer_sizes": {"mode": "top_middle_bottom", "top": 0.6, "middle": 0.6, "bottom": 0.6}
    }))
    .unwrap();
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let copper = [CopperLayer::Top, inner, CopperLayer::Bottom];
    assert_eq!(via.min_diameter(&copper), 0.6);
    // A full list that leaves Inner 1 out keeps the via's own size there.
    via.layer_sizes = Some(LayerSizes::Full {
        layers: vec![
            LayerSize::new(CopperLayer::Top, 0.6),
            LayerSize::new(CopperLayer::Bottom, 0.6),
        ],
    });
    assert_eq!(via.min_diameter(&copper), 0.45);
    // A blind via from Top to Inner 1 never reaches the bottom's size.
    via.layers = vec!["TopCopper".into(), "Inner1Copper".into()];
    via.layer_sizes = Some(LayerSizes::Full {
        layers: vec![
            LayerSize::new(CopperLayer::Top, 0.6),
            LayerSize::new(inner, 0.6),
            LayerSize::new(CopperLayer::Bottom, 0.3),
        ],
    });
    assert_eq!(via.min_diameter(&copper), 0.6);
}

#[test]
fn a_size_list_the_file_cannot_hold_has_a_fault() {
    let empty: LayerSizes<f64> = LayerSizes::Full { layers: Vec::new() };
    assert_eq!(empty.list_fault(), Some(LayerSizesFault::NoLayers));
    let twice = LayerSizes::Full {
        layers: vec![
            LayerSize::new(CopperLayer::Top, 0.6),
            LayerSize::new(CopperLayer::Top, 0.5),
        ],
    };
    assert_eq!(
        twice.list_fault(),
        Some(LayerSizesFault::NamedTwice("TopCopper".into()))
    );
    assert!(size_storable(0.000_001));
    assert!(!size_storable(0.000_000_4));
    assert!(!size_storable(f64::NAN));
    let number = |t: &str| t.parse::<f64>().ok();
    assert!(parse_layer_sizes(";", number).is_err());
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

#[test]
fn a_mode_change_keeps_each_layers_size() {
    let inner = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let copper = [CopperLayer::Top, inner, CopperLayer::Bottom];
    let tmb = sizes_in_mode(None, 0.6, StackMode::TopMiddleBottom, &copper).unwrap();
    assert_eq!(tmb.sizes(), [0.6, 0.6, 0.6]);
    let mut tmb = tmb;
    tmb.set(inner, 0.45);
    let full = sizes_in_mode(Some(&tmb), 0.6, StackMode::Full, &copper).unwrap();
    assert_eq!(full.mode(), StackMode::Full);
    assert_eq!(full.on(inner), Some(0.45));
    assert_eq!(full.on(CopperLayer::Top), Some(0.6));
    assert_eq!(
        sizes_in_mode(Some(&full), 0.6, StackMode::Simple, &copper),
        None
    );
}

#[test]
fn sizes_parse_from_text() {
    let number = |t: &str| t.parse::<f64>().ok();
    assert_eq!(parse_layer_sizes("", number), Ok(None));
    assert_eq!(
        parse_layer_sizes("top=0.6;middle=0.45;bottom=0.6", number),
        Ok(Some(LayerSizes::TopMiddleBottom {
            top: 0.6,
            middle: 0.45,
            bottom: 0.6
        }))
    );
    let full = parse_layer_sizes("TopCopper=0.6; Inner1Copper=0.4", number)
        .unwrap()
        .unwrap();
    assert_eq!(full.on_token("Inner1Copper"), Some(0.4));
    assert!(parse_layer_sizes("TopSilkscreen=0.6", number).is_err());
    assert!(parse_layer_sizes("top=x", number).is_err());
}

/// Mockup rows 176 and 177: a derived stack carries no finish and no Df, so
/// it needs no newer token; either one does.
#[test]
fn a_finish_or_a_loss_tangent_is_materials_data() {
    let stack = LayerStack::derived(&[], 1.6);
    assert!(!stack.has_materials_data());
    assert!(stack.layers.iter().all(|l| l.loss_tangent == 0.0));

    let mut with_df = stack.clone();
    with_df.layers[4].loss_tangent = 0.02;
    assert!(with_df.has_materials_data());

    let mut finished = stack.clone();
    finished.finish = Some(BoardFinish::default());
    assert!(finished.has_materials_data());
    assert_eq!(stack.shown_finish(), BoardFinish::default());
}

/// Mockup row 105: the default finish is ENIG with green LPI glossy mask on
/// both sides and white silkscreen; a negative mask thickness is invalid.
#[test]
fn the_default_finish_matches_the_mockup_and_checks_its_numbers() {
    let finish = BoardFinish::default();
    assert_eq!(finish.surface, SurfaceFinish::Enig);
    assert_eq!(finish.mask.kind, MaskType::Lpi);
    assert_eq!(finish.mask.color, FinishColor::Green);
    assert_eq!(finish.mask.gloss, MaskGloss::Glossy);
    assert_eq!(finish.mask.sides, FinishSides::Both);
    assert_eq!(finish.silkscreen.color, FinishColor::White);
    assert!(finish.numbers_valid());
    assert!(SurfaceFinish::LeadFreeHasl.is_hasl() && !SurfaceFinish::Enig.is_hasl());

    let mut bad = finish;
    bad.mask.over_laminate_mm = -0.01;
    assert!(!bad.numbers_valid());
    bad.mask.over_laminate_mm = f64::INFINITY;
    assert!(!bad.numbers_valid());
}
