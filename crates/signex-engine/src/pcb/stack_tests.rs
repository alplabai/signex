//! #2401: the layer stack edit, the copper names it moves and its undo.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_stack::{LayerStack, StackLayer, StackLayerKind};
use uuid::Uuid;

use crate::pcb::{BoardCommand, BoardCommandError, BoardPatchKind};

/// `copper` layers listed (Top, Inner 1.., Bottom), a silkscreen layer, and
/// one track on `track_layer`.
fn board(copper: usize, track_layer: &str) -> PcbBoard {
    let mut layers = vec![serde_json::json!({"id": 50, "name": "TopSilkscreen", "layer_type": ""})];
    for i in 0..copper {
        let name =
            signex_types::format::pcb_layer_token(signex_types::pcb_layer::PcbLayer::Copper(
                signex_types::pcb_stack::copper_at(i, copper).unwrap(),
            ));
        layers.push(serde_json::json!({"id": i, "name": name, "layer_type": "signal"}));
    }
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": layers,
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 1.0, "y": 0.0}, "width": 0.25, "layer": track_layer}]
    }))
    .unwrap()
}

fn snapshot(board: &PcbBoard) -> String {
    serde_json::to_string(board).unwrap()
}

fn copper_names(board: &PcbBoard) -> Vec<String> {
    board
        .layers
        .iter()
        .filter(|d| d.name.ends_with("Copper"))
        .map(|d| d.name.clone())
        .collect()
}

/// The index of the `n`-th copper row of `stack`.
fn copper_row(stack: &LayerStack, n: usize) -> usize {
    stack
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind.is_copper())
        .nth(n)
        .map(|(i, _)| i)
        .unwrap()
}

/// A dielectric and a new signal layer below the `n`-th copper row.
fn add_inner_below(stack: &mut LayerStack, n: usize) {
    let at = copper_row(stack, n) + 1;
    stack
        .layers
        .insert(at, StackLayer::new(StackLayerKind::Signal));
    stack
        .layers
        .insert(at, StackLayer::new(StackLayerKind::Dielectric));
}

fn set(board: &mut PcbBoard, stack: LayerStack) -> Result<crate::pcb::Applied, BoardCommandError> {
    BoardCommand::SetLayerStack {
        stack: Box::new(stack),
    }
    .apply(board)
}

#[test]
fn two_new_inner_layers_join_the_layer_list_and_the_total() {
    let mut b = board(2, "TopCopper");
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    add_inner_below(&mut stack, 0);
    add_inner_below(&mut stack, 1);
    let total = stack.total_thickness_mm();
    let applied = set(&mut b, stack).unwrap();
    assert_eq!(applied.patch.kind, BoardPatchKind::LayerStackChanged);
    assert_eq!(
        copper_names(&b),
        ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
    );
    assert_eq!(b.layers[0].name, "TopSilkscreen", "other layers keep place");
    assert_eq!(b.thickness, total, "the board is as thick as its stack");
    let stored = b.stackup.as_ref().unwrap();
    let tokens: Vec<&str> = stored.copper().map(|l| l.layer.as_str()).collect();
    assert_eq!(
        tokens,
        ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
    );
    let after = snapshot(&b);
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "undo is exact");
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), after, "redo is exact");
}

#[test]
fn a_layer_added_above_an_inner_layer_moves_its_copper_down() {
    let mut b = board(4, "Inner2Copper");
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    add_inner_below(&mut stack, 1);
    let applied = set(&mut b, stack).unwrap();
    assert_eq!(
        b.segments[0].layer, "Inner3Copper",
        "the track moves with its layer"
    );
    assert_eq!(copper_names(&b).len(), 5);
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before);
}

#[test]
fn a_layer_with_items_on_it_cannot_be_removed() {
    let mut b = board(4, "Inner2Copper");
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    let inner2 = copper_row(&stack, 2);
    stack.layers.remove(inner2);
    assert_eq!(
        set(&mut b, stack.clone()),
        Err(BoardCommandError::LayerInUse {
            layer: "Inner2Copper".into(),
            items: 1
        })
    );
    assert_eq!(snapshot(&b), before, "a refusal changes nothing");

    b.segments[0].layer = "TopCopper".into();
    set(&mut b, stack).unwrap();
    assert_eq!(
        copper_names(&b),
        ["TopCopper", "Inner1Copper", "BottomCopper"]
    );
}

#[test]
fn bad_stacks_are_refused() {
    let b = board(2, "TopCopper");
    let base = LayerStack::derived(&b.layers, b.thickness);

    let mut one_copper = base.clone();
    one_copper.layers.remove(copper_row(&base, 1));
    assert_eq!(
        set(&mut b.clone(), one_copper),
        Err(BoardCommandError::CopperLayerCount(1))
    );

    let mut negative = base.clone();
    negative.layers[0].thickness_mm = -1.0;
    assert_eq!(
        set(&mut b.clone(), negative),
        Err(BoardCommandError::StackRowInvalid { row: 0 })
    );

    let mut twice = base.clone();
    let bottom = copper_row(&base, 1);
    twice.layers[bottom].layer = "TopCopper".into();
    assert_eq!(
        set(&mut b.clone(), twice),
        Err(BoardCommandError::DuplicateStackLayer("TopCopper".into()))
    );

    let mut moved = base.clone();
    let top = copper_row(&base, 0);
    let row = moved.layers.remove(top);
    let bottom = copper_row(&moved, 0);
    moved.layers.insert(bottom + 1, row);
    assert!(matches!(
        set(&mut b.clone(), moved),
        Err(BoardCommandError::OuterCopperMoved(_))
    ));
}
