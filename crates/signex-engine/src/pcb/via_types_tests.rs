use pcb_geom::ItemRef;
use signex_types::pcb::{
    LayerDef, PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM, PcbBoard, Point, Via, ViaType,
};
use signex_types::pcb_layer::{CopperLayer, InnerCopper};
use uuid::Uuid;

use super::*;
use crate::pcb::tests::board;

fn inner1() -> CopperLayer {
    CopperLayer::Inner(InnerCopper::new(1).expect("inner 1"))
}

fn via(n: u128, kind: ViaType, layers: &[&str], size: (f64, f64)) -> Via {
    Via {
        uuid: Uuid::from_u128(n),
        position: Point::new(n as f64, 0.0),
        diameter: size.0,
        drill: size.1,
        layers: layers.iter().map(|l| (*l).into()).collect(),
        net: 0,
        via_type: kind,
        locked: false,
        layer_sizes: None,
        attributes: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn through(n: u128, size: (f64, f64)) -> Via {
    via(n, ViaType::Through, &["TopCopper", "BottomCopper"], size)
}

fn blind(n: u128, size: (f64, f64)) -> Via {
    via(n, ViaType::Blind, &["TopCopper", "Inner1Copper"], size)
}

/// The engine fixture with four copper layers and `vias` only.
fn four_layer_board(vias: Vec<Via>) -> PcbBoard {
    let mut b = board();
    b.layers = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
        .iter()
        .enumerate()
        .map(|(i, name)| LayerDef {
            id: i as u8,
            name: (*name).into(),
            layer_type: "signal".into(),
            display_name: String::new(),
        })
        .collect();
    b.vias = vias;
    b
}

fn run(b: &mut PcbBoard, commands: Vec<BoardCommand>) {
    assert_eq!(commands.len(), 1, "one command, one undo step");
    for c in commands {
        c.apply(b).expect("applies");
    }
}

#[test]
fn a_through_only_board_has_one_type_per_tenting_sized_by_most_vias() {
    let mut tented = through(5, (0.6, 0.3));
    tented.attributes.top.tented = true;
    tented.attributes.bottom.tented = true;
    let mut b = board();
    b.vias = vec![
        through(1, (0.8, 0.4)),
        through(2, (0.6, 0.3)),
        through(3, (0.6, 0.3)),
        through(4, (0.8, 0.4)),
        through(6, (0.6, 0.3)),
        tented,
    ];
    let types = via_types(&b);
    assert_eq!(types.len(), 2, "{types:?}");
    let open = &types[0];
    assert_eq!(
        (open.kind, open.top, open.bottom),
        (ViaType::Through, CopperLayer::Top, CopperLayer::Bottom)
    );
    assert_eq!(
        open.span,
        (1, 2),
        "the fixture lists no layers: Top and Bottom"
    );
    assert_eq!(
        (open.diameter, open.drill),
        (0.6, 0.3),
        "three of five vias"
    );
    assert!(!open.protection.tented_top && !open.protection.tented_bottom);
    let both = &types[1];
    assert!(both.protection.tented_top && both.protection.tented_bottom);
    assert_eq!(both.key().kind, ViaType::Through);
}

#[test]
fn a_tie_takes_the_smaller_size() {
    let mut b = board();
    b.vias = vec![through(1, (0.8, 0.4)), through(2, (0.6, 0.3))];
    let types = via_types(&b);
    assert_eq!((types[0].diameter, types[0].drill), (0.6, 0.3));
}

#[test]
fn a_blind_pair_is_a_type_after_the_through_ones() {
    let b = four_layer_board(vec![
        blind(1, (0.3, 0.15)),
        through(2, (0.6, 0.3)),
        blind(3, (0.3, 0.15)),
    ]);
    let types = via_types(&b);
    assert_eq!(types.len(), 2, "{types:?}");
    assert_eq!((types[0].kind, types[0].span), (ViaType::Through, (1, 4)));
    let pair = &types[1];
    assert_eq!(
        (pair.kind, pair.top, pair.bottom),
        (ViaType::Blind, CopperLayer::Top, inner1())
    );
    assert_eq!(pair.span, (1, 2));
    assert_eq!((pair.diameter, pair.drill), (0.3, 0.15));
}

#[test]
fn a_board_without_a_through_via_gets_the_via_tools_type() {
    let b = four_layer_board(vec![blind(1, (0.3, 0.15))]);
    let types = via_types(&b);
    assert_eq!(types.len(), 2, "{types:?}");
    let tool = &types[0];
    assert_eq!((tool.kind, tool.span), (ViaType::Through, (1, 4)));
    assert_eq!(
        (tool.diameter, tool.drill),
        (PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM),
        "no setup: the PCB defaults the Via tool places"
    );
}

#[test]
fn a_via_finds_its_type_and_an_own_size_is_an_override() {
    let b = four_layer_board(vec![
        through(1, (0.6, 0.3)),
        through(2, (0.6, 0.3)),
        through(3, (0.8, 0.3)),
        blind(4, (0.3, 0.15)),
    ]);
    let types = via_types(&b);
    assert_eq!(type_of(&types, &b.vias[0]), Some(0));
    assert_eq!(type_of(&types, &b.vias[3]), Some(1));
    assert!(!is_overridden(&types[0], &b.vias[0]));
    assert!(is_overridden(&types[0], &b.vias[2]), "its diameter differs");
    let mut tented = b.vias[0].clone();
    tented.attributes.top.tented = true;
    assert_eq!(type_of(&types, &tented), None, "no tented through type");
}

#[test]
fn applying_a_type_sets_span_kind_size_and_tenting_in_one_command() {
    let mut tented = through(1, (0.8, 0.4));
    tented.attributes.top.tented = true;
    let mut b = four_layer_board(vec![tented, blind(2, (0.3, 0.15))]);
    let types = via_types(&b);
    let pair = types
        .iter()
        .find(|t| t.kind == ViaType::Blind)
        .unwrap()
        .clone();
    let items = [ItemRef::Via(Uuid::from_u128(1))];
    let commands = apply_type_commands(&b, &items, &pair).unwrap();
    run(&mut b, commands);
    let v = &b.vias[0];
    assert_eq!(v.via_type, ViaType::Blind);
    assert_eq!(v.layers, ["TopCopper", "Inner1Copper"]);
    assert_eq!((v.diameter, v.drill), (0.3, 0.15));
    assert!(!v.attributes.top.tented && !v.attributes.bottom.tented);
    assert_eq!(
        type_of(&via_types(&b), v),
        types.iter().position(|t| *t == pair)
    );
}

#[test]
fn a_micro_type_makes_a_through_via_micro() {
    let micro = via(
        2,
        ViaType::Micro,
        &["TopCopper", "Inner1Copper"],
        (0.25, 0.1),
    );
    let mut b = four_layer_board(vec![through(1, (0.6, 0.3)), micro]);
    let types = via_types(&b);
    let t = types
        .iter()
        .find(|t| t.kind == ViaType::Micro)
        .unwrap()
        .clone();
    let items = [ItemRef::Via(Uuid::from_u128(1))];
    let commands = apply_type_commands(&b, &items, &t).unwrap();
    run(&mut b, commands);
    assert_eq!(b.vias[0].via_type, ViaType::Micro);
    assert_eq!((b.vias[0].diameter, b.vias[0].drill), (0.25, 0.1));
}

#[test]
fn resetting_an_override_restores_only_diameter_and_drill() {
    let mut b = four_layer_board(vec![
        through(1, (0.6, 0.3)),
        through(2, (0.6, 0.3)),
        through(3, (0.9, 0.45)),
    ]);
    b.vias[2].net = 7;
    let types = via_types(&b);
    let items = [ItemRef::Via(Uuid::from_u128(3))];
    let commands = reset_override_commands(&b, &items, &types[0]).unwrap();
    run(&mut b, commands);
    let v = &b.vias[2];
    assert_eq!((v.diameter, v.drill), (0.6, 0.3));
    assert_eq!((v.via_type, v.net), (ViaType::Through, 7));
    assert!(!is_overridden(&types[0], v));
    assert_eq!(
        reset_override_commands(&b, &items, &types[0]),
        Ok(Vec::new()),
        "nothing left to reset"
    );
}

#[test]
fn a_locked_via_refuses_a_type() {
    let mut b = four_layer_board(vec![through(1, (0.6, 0.3)), blind(2, (0.3, 0.15))]);
    b.vias[0].locked = true;
    let types = via_types(&b);
    let items = [ItemRef::Via(Uuid::from_u128(1))];
    assert_eq!(
        apply_type_commands(&b, &items, &types[1]),
        Err(PrimitiveEditRefusal::ItemLocked)
    );
    assert_eq!(
        apply_type_commands(&b, &[], &types[1]),
        Err(PrimitiveEditRefusal::Empty)
    );
}

#[test]
fn the_setup_sizes_the_tool_type_and_sets_the_minimum_drill() {
    let mut b = four_layer_board(Vec::new());
    assert_eq!(min_via_drill(&b), None, "no setup, no minimum");
    b.setup = Some(signex_types::pcb::PcbSetup {
        grid_size: 0.0,
        trace_width: 0.0,
        via_diameter: 0.7,
        via_drill: 0.35,
        clearance: 0.0,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.2,
    });
    assert_eq!(min_via_drill(&b), Some(0.2));
    let types = via_types(&b);
    assert_eq!(types.len(), 1, "{types:?}");
    assert_eq!((types[0].diameter, types[0].drill), (0.7, 0.35));
}

#[test]
fn the_finished_hole_is_the_drill_less_the_plating() {
    assert!((ViaTypeDef::finished_hole(0.3) - 0.25).abs() < 1e-12);
    assert_eq!(ViaTypeDef::finished_hole(0.01), 0.0);
}

/// The trap PCB_LAYER_STACK.md "Via types" names: a derived type takes the
/// size most of its vias share, so overriding most of them moves the type
/// itself, and the vias left alone then read as overridden. Pinned until
/// via types are stored (mockup row 105).
#[test]
fn overriding_most_vias_of_a_derived_type_moves_the_type() {
    let mut b = board();
    b.vias = (1..=5).map(|n| through(n, (0.6, 0.3))).collect();
    for v in b.vias.iter_mut().take(3) {
        (v.diameter, v.drill) = (0.8, 0.4);
    }
    let types = via_types(&b);
    assert_eq!(types.len(), 1, "{types:?}");
    assert_eq!((types[0].diameter, types[0].drill), (0.8, 0.4));
    assert!(!is_overridden(&types[0], &b.vias[0]), "an edited via");
    assert!(is_overridden(&types[0], &b.vias[4]), "an untouched via");

    // A type of one via is that via's size: it never reads as overridden.
    b.vias = vec![through(1, (0.7, 0.35))];
    let types = via_types(&b);
    assert!(!is_overridden(&types[0], &b.vias[0]));
}
