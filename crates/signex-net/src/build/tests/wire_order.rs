//! The derived netlist must be a function of the geometry, never of the order
//! wires happen to sit in the document (issue #402).
//!
//! Two independent order dependences used to leak through:
//!
//! 1. Anchoring a point (label / sheet pin) walked the wire slice and unioned
//!    into the **first** match. At a junction-less T — where the point is one
//!    wire's own endpoint *and* another wire's interior — that either bridged
//!    the two wires or did nothing, purely by slice order.
//! 2. `uf::union` made the second argument's root the class representative, so
//!    the representative depended on union order, and `build_netlist` numbers
//!    nets by sorted root.
//!
//! The `#399` label-order tests never caught either: they vary label order with
//! `wires` pinned, so the anchor loop always saw the same first match.

use super::*;

/// `build_netlist` over `sheet`, then again with the wire slice reversed.
fn both_wire_orders(sheet: &SchematicSheet) -> (Netlist, Netlist) {
    let forward = build_netlist(sheet);
    let mut flipped = sheet.clone();
    flipped.wires.reverse();
    (forward, build_netlist(&flipped))
}

/// A = (0,0)–(10,0) and B = (5,0)–(5,10): B *ends* where A's interior runs, with
/// no junction dot. A label sits on that T point, so it is simultaneously one
/// wire's endpoint and the other's interior — the exact ambiguity.
fn junction_less_t() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 10.0)));
    sheet
        .labels
        .push(label("TEE", pt(5.0, 0.0), LabelType::Net));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "R1", "R", pt(0.0, 0.0));
    place(&mut sheet, "R2", "R", pt(5.0, 10.0));
    sheet
}

#[test]
fn netlist_is_independent_of_wire_order_at_a_junction_less_t() {
    let sheet = junction_less_t();
    let (forward, reversed) = both_wire_orders(&sheet);

    assert_eq!(
        forward, reversed,
        "same geometry, reversed wire order, different netlist"
    );
    // No junction dot ⇒ no connection (issue #107). Anchoring the label may
    // attach it to a wire, but it must never assert a wire-to-wire join.
    assert_eq!(
        forward.nets.len(),
        2,
        "a junction-less T must stay two nets: {:?}",
        forward.nets
    );
}

#[test]
fn a_junction_merges_the_t_in_either_wire_order() {
    // The other side of the #107 semantics: the dot *is* what connects.
    let mut sheet = junction_less_t();
    sheet.junctions.push(junction(pt(5.0, 0.0)));
    let (forward, reversed) = both_wire_orders(&sheet);

    assert_eq!(forward, reversed);
    assert_eq!(forward.nets.len(), 1, "{:?}", forward.nets);
    assert_eq!(forward.nets[0].name, "TEE");
}

#[test]
fn net_ids_are_independent_of_wire_order() {
    // Net X is two wires sharing the endpoint (10,0); net Y is a separate wire
    // whose root sorts *between* X's two possible order-dependent roots, so a
    // representative that follows union order permutes the two `NetId`s (and
    // with them the `N$k` names).
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(10.0, 0.0), pt(50.0, 0.0)));
    sheet.wires.push(wire(pt(10.0, 0.0), pt(10.0, 10.0)));
    sheet.wires.push(wire(pt(20.0, 20.0), pt(30.0, 20.0)));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "RX", "R", pt(50.0, 0.0));
    place(&mut sheet, "RY", "R", pt(20.0, 20.0));

    let (forward, reversed) = both_wire_orders(&sheet);
    assert_eq!(forward.nets.len(), 2, "{:?}", forward.nets);
    assert_eq!(
        forward, reversed,
        "net ids / names permuted with wire order"
    );
}

#[test]
fn a_label_on_an_x_crossing_picks_the_same_wire_in_either_order() {
    // The milder cousin: two wires merely cross at (5,5) with no junction, and
    // a label sits on the pure interior of both. Whichever wire the label names
    // must not depend on document order, and the crossing must not connect.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire(pt(0.0, 5.0), pt(10.0, 5.0)));
    sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 10.0)));
    sheet
        .labels
        .push(label("CROSS", pt(5.0, 5.0), LabelType::Net));
    add_lib(
        &mut sheet,
        "R",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
    );
    place(&mut sheet, "RH", "R", pt(0.0, 5.0));
    place(&mut sheet, "RV", "R", pt(5.0, 0.0));

    let (forward, reversed) = both_wire_orders(&sheet);
    assert_eq!(forward, reversed, "the label switched wires with the order");
    assert_eq!(
        forward.nets.len(),
        2,
        "a bare crossing is not a connection: {:?}",
        forward.nets
    );
    assert_eq!(
        forward.nets.iter().filter(|n| n.name == "CROSS").count(),
        1,
        "exactly one net carries the label"
    );
}
