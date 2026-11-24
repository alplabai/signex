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

fn set_name(b: &mut PcbBoard, name: Option<&str>) -> crate::pcb::Applied {
    BoardCommand::SetStackupName {
        name: name.map(str::to_string),
    }
    .apply(b)
    .unwrap()
}

/// Decision 2026-11-22 (mockup row 105): naming the stackup is one exact
/// undo step that changes nothing else.
#[test]
fn a_stackup_name_is_one_undo_step_and_leaves_the_copper_alone() {
    let mut b = board(2, "TopCopper");
    let before = snapshot(&b);
    let (layers, thickness) = (b.layers.clone(), b.thickness);
    let applied = set_name(&mut b, Some("  sensor-node 4L "));
    assert_eq!(applied.patch.kind, BoardPatchKind::LayerStackChanged);
    let stored = b.stackup.as_ref().unwrap();
    assert_eq!(stored.name.as_deref(), Some("sensor-node 4L"), "trimmed");
    assert_eq!(
        stored.layers,
        LayerStack::derived(&layers, thickness).layers,
        "the rows are the ones the board showed"
    );
    assert_eq!(b.layers, layers);
    assert_eq!(b.thickness, thickness);
    let after = snapshot(&b);
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "undo is exact");
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), after, "redo is exact");
}

/// A whole-stack edit (the manager, `set_layer_stack`) cannot change the name.
#[test]
fn a_stack_edit_keeps_the_stackup_name() {
    let mut b = board(2, "TopCopper");
    set_name(&mut b, Some("4L"));
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    assert_eq!(stack.name, None);
    add_inner_below(&mut stack, 0);
    set(&mut b, stack).unwrap();
    assert_eq!(b.stackup.as_ref().unwrap().name.as_deref(), Some("4L"));
}

/// A name taken off a stack nobody edited leaves no stored stack, so the
/// file goes back to what it was; an edited stack stays stored.
#[test]
fn clearing_the_name_of_an_unedited_stack_stores_no_stack() {
    let mut b = board(2, "TopCopper");
    set_name(&mut b, Some("4L"));
    set_name(&mut b, Some("   "));
    assert!(b.stackup.is_none());

    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    add_inner_below(&mut stack, 0);
    set(&mut b, stack).unwrap();
    set_name(&mut b, Some("4L"));
    set_name(&mut b, None);
    let stored = b.stackup.as_ref().expect("the edited rows stay");
    assert_eq!(stored.name, None);
    assert_eq!(stored.copper_count(), 3);
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

/// A through via with a size on each of `layers`, inner ones smaller.
fn full_via(layers: &[&str]) -> signex_types::pcb::Via {
    let sizes: Vec<serde_json::Value> = layers
        .iter()
        .enumerate()
        .map(|(i, l)| serde_json::json!({"layer": l, "size": 0.6 - 0.05 * i as f64}))
        .collect();
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(9), "position": {"x": 0.0, "y": 0.0},
        "diameter": 0.6, "drill": 0.2, "layers": ["TopCopper", "BottomCopper"],
        "layer_sizes": {"mode": "full", "layers": sizes}
    }))
    .unwrap()
}

#[test]
fn removing_an_inner_layer_drops_its_full_sizes_and_undo_puts_them_back() {
    let mut b = board(4, "TopCopper");
    b.vias.push(full_via(&[
        "TopCopper",
        "Inner1Copper",
        "Inner2Copper",
        "BottomCopper",
    ]));
    let inner2 = b.vias[0].layer_sizes.as_ref().unwrap().sizes()[2];
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    stack.layers.remove(copper_row(&stack, 1));
    let applied = set(&mut b, stack).unwrap();

    let sizes = b.vias[0].layer_sizes.clone().unwrap();
    let signex_types::pcb::LayerSizes::Full { layers } = &sizes else {
        panic!("still a full list: {sizes:?}");
    };
    let names: Vec<&str> = layers.iter().map(|l| l.layer.as_str()).collect();
    assert_eq!(
        names,
        ["TopCopper", "Inner1Copper", "BottomCopper"],
        "Inner 1's entry is gone and Inner 2's moved up, each named once"
    );
    assert_eq!(sizes.on_token("Inner1Copper"), Some(inner2));
    signex_types::format::SnxPcb::new(b.clone())
        .write_string()
        .expect("the board saves");

    let after = snapshot(&b);
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "undo is exact");
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), after, "redo is exact");
}

/// #2840: a replay must not delete sizes edited since on a layer it removes.
#[test]
fn a_stack_replay_is_refused_while_an_item_holds_sizes_on_a_layer_it_removes() {
    let mut b = board(4, "TopCopper");
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    stack.layers.remove(copper_row(&stack, 1));
    let command = BoardCommand::SetLayerStack {
        stack: Box::new(stack),
    };
    let guard = crate::pcb::BoardReplayGuard::before(&b, &[command]);
    assert!(!guard.drops_layer_sizes(&b));
    assert!(guard.holds_as_found(&b));

    b.vias.push(full_via(&[
        "TopCopper",
        "Inner1Copper",
        "Inner2Copper",
        "BottomCopper",
    ]));

    assert!(guard.drops_layer_sizes(&b));
    assert!(!guard.holds_as_found(&b));
    assert!(!guard.holds(&b));
}

#[test]
fn a_full_list_left_with_no_entry_becomes_one_size() {
    let mut b = board(3, "TopCopper");
    b.vias.push(full_via(&["Inner1Copper"]));
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    stack.layers.remove(copper_row(&stack, 1));
    let applied = set(&mut b, stack).unwrap();
    assert_eq!(b.vias[0].layer_sizes, None);
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before);
}

#[test]
fn a_stack_edit_that_keeps_the_copper_leaves_the_layer_list_alone() {
    let mut b = board(4, "TopCopper");
    // The silkscreen between two copper entries, as a user may order tabs.
    let silk = b.layers.remove(0);
    b.layers.insert(2, silk);
    let layers = b.layers.clone();
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    let dielectric = stack
        .layers
        .iter()
        .position(|l| l.kind == StackLayerKind::Dielectric)
        .unwrap();
    stack.layers[dielectric].thickness_mm += 0.1;
    set(&mut b, stack).unwrap();
    assert_eq!(b.layers, layers, "the tab order is kept");
}

/// A free copper arc on `layer`, net 3.
fn free_arc(layer: &str) -> signex_types::pcb_arc::TrackArc {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(20), "start": {"x": 0.0, "y": 0.0},
        "mid": {"x": 1.0, "y": 1.0}, "end": {"x": 2.0, "y": 0.0},
        "width": 0.25, "layer": layer, "net": 3
    }))
    .unwrap()
}

/// A free through pad on `layers` with a size on each of `sized`, inner
/// ones smaller.
fn free_pad(layers: &[&str], sized: &[&str]) -> signex_types::pcb::Pad {
    let sizes: Vec<serde_json::Value> = sized
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let s = 1.6 - 0.1 * i as f64;
            serde_json::json!({"layer": l, "size": {"x": s, "y": s}})
        })
        .collect();
    let mut pad = serde_json::json!({
        "uuid": Uuid::from_u128(21), "number": "1", "pad_type": "thru",
        "shape": "circle", "position": {"x": 5.0, "y": 5.0},
        "size": {"x": 1.6, "y": 1.6}, "drill": null, "layers": layers, "net": null
    });
    if !sizes.is_empty() {
        pad["layer_sizes"] = serde_json::json!({"mode": "full", "layers": sizes});
    }
    serde_json::from_value(pad).unwrap()
}

#[test]
fn a_layer_added_above_an_inner_layer_moves_free_copper_down() {
    let mut b = board(4, "TopCopper");
    b.free.arcs.push(free_arc("Inner2Copper"));
    b.free.pads.push(free_pad(
        &["Inner2Copper"],
        &["Inner1Copper", "Inner2Copper"],
    ));
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    add_inner_below(&mut stack, 1);
    let applied = set(&mut b, stack).unwrap();
    assert_eq!(
        b.free.arcs[0].layer, "Inner3Copper",
        "the free arc moves with its layer"
    );
    let pad = &b.free.pads[0];
    assert_eq!(pad.layers, ["Inner3Copper"], "the free pad moves too");
    let sizes = pad.layer_sizes.as_ref().unwrap();
    assert!(sizes.on_token("Inner3Copper").is_some());
    assert!(sizes.on_token("Inner2Copper").is_none());
    let after = snapshot(&b);
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "undo is exact");
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), after, "redo is exact");
}

#[test]
fn a_layer_with_a_free_arc_or_pad_on_it_cannot_be_removed() {
    let refused = |b: &mut PcbBoard| {
        let before = snapshot(b);
        let mut stack = LayerStack::derived(&b.layers, b.thickness);
        stack.layers.remove(copper_row(&stack, 2));
        assert_eq!(
            set(b, stack),
            Err(BoardCommandError::LayerInUse {
                layer: "Inner2Copper".into(),
                items: 1
            })
        );
        assert_eq!(snapshot(b), before, "a refusal changes nothing");
    };
    let mut arc = board(4, "TopCopper");
    arc.free.arcs.push(free_arc("Inner2Copper"));
    refused(&mut arc);
    let mut pad = board(4, "TopCopper");
    pad.free.pads.push(free_pad(&["Inner2Copper"], &[]));
    refused(&mut pad);
}

#[test]
fn removing_an_inner_layer_drops_a_free_pads_sizes_and_undo_puts_them_back() {
    let mut b = board(4, "TopCopper");
    let copper = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"];
    b.free
        .pads
        .push(free_pad(&["TopCopper", "BottomCopper"], &copper));
    let inner2 = b.free.pads[0]
        .layer_sizes
        .as_ref()
        .unwrap()
        .on_token("Inner2Copper")
        .unwrap();
    let before = snapshot(&b);
    let mut stack = LayerStack::derived(&b.layers, b.thickness);
    stack.layers.remove(copper_row(&stack, 1));
    let applied = set(&mut b, stack).unwrap();

    let sizes = b.free.pads[0].layer_sizes.clone().unwrap();
    let signex_types::pcb::LayerSizes::Full { layers } = &sizes else {
        panic!("still a full list: {sizes:?}");
    };
    let names: Vec<&str> = layers.iter().map(|l| l.layer.as_str()).collect();
    assert_eq!(
        names,
        ["TopCopper", "Inner1Copper", "BottomCopper"],
        "Inner 1's entry is gone and Inner 2's moved up"
    );
    assert_eq!(sizes.on_token("Inner1Copper"), Some(inner2));

    let after = snapshot(&b);
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before, "undo is exact");
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), after, "redo is exact");
}
