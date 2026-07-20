use super::*;
use crate::context::{ErcJunction, ErcLabel, ErcPin, ErcSymbol, ErcWire, PaperSize};
use signex_types::schematic::{LabelType, PinDirection, Point};
use std::collections::HashMap;
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn wire(a: Point, b: Point) -> ErcWire {
    ErcWire {
        uuid: Uuid::nil(),
        start: a,
        end: b,
    }
}

fn power_label(text: &str, pos: Point) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::nil(),
        text: text.into(),
        position: pos,
        label_type: LabelType::Power,
    }
}

fn power_port(value: &str, pos: Point) -> ErcSymbol {
    ErcSymbol {
        uuid: Uuid::nil(),
        reference: "#PWR01".into(),
        value: value.into(),
        position: pos,
        is_power: true,
        pins: Vec::new(),
        attrs: HashMap::new(),
    }
}

fn net_label(text: &str, pos: Point) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::nil(),
        text: text.into(),
        position: pos,
        label_type: LabelType::Net,
    }
}

fn global_label(text: &str, pos: Point) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::nil(),
        text: text.into(),
        position: pos,
        label_type: LabelType::Global,
    }
}

/// A non-power symbol with a single pin, its `connected` flag set
/// exactly as `context::point_is_connected` would compute it — these
/// tests build `ErcContext` by hand (below the projection step), so the
/// flag is the fixture's job here.
fn symbol_with_pin(reference: &str, world_pos: Point, connected: bool) -> ErcSymbol {
    ErcSymbol {
        uuid: Uuid::nil(),
        reference: reference.into(),
        value: String::new(),
        position: world_pos,
        is_power: false,
        pins: vec![ErcPin {
            world_pos,
            electrical_type: PinDirection::Passive,
            required: true,
            connected,
        }],
        attrs: HashMap::new(),
    }
}

fn ctx(
    wires: Vec<ErcWire>,
    junctions: Vec<ErcJunction>,
    labels: Vec<ErcLabel>,
    symbols: Vec<ErcSymbol>,
) -> ErcContext {
    ErcContext {
        paper_size: PaperSize::A4,
        symbols,
        wires,
        buses: Vec::new(),
        labels,
        junctions,
        no_connects: Vec::new(),
        bus_entries: Vec::new(),
        child_sheets: Vec::new(),
        nets: Vec::new(),
        children: HashMap::new(),
    }
}

#[test]
fn missing_power_flag_honors_a_t_junction() {
    // A +3V3 port at (10,0) on the horizontal wire; a matching +3V3 label at
    // (5,5) on a vertical wire that ends on the horizontal's interior, joined
    // by a junction at (5,0). Port and label share a net only *through* the
    // T-junction — which the shared connectivity merges, so the port is
    // cross-referenced and NOT flagged. (The old inline pass only `find`-ed
    // junctions, split the net, and false-flagged this.)
    let c = ctx(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(5.0, 0.0), pt(5.0, 5.0)),
        ],
        vec![ErcJunction {
            position: pt(5.0, 0.0),
        }],
        vec![power_label("+3V3", pt(5.0, 5.0))],
        vec![power_port("+3V3", pt(10.0, 0.0))],
    );
    let mut out = Vec::new();
    missing_power_flag(&c, &mut out);
    assert!(
        out.is_empty(),
        "port cross-referenced through the T-junction: {out:?}"
    );
}

#[test]
fn missing_power_flag_fires_when_no_same_net_label() {
    // Same port, but the +3V3 label sits on a disjoint wire with nothing
    // tying it to the port's net → not cross-referenced → flagged.
    let c = ctx(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(50.0, 0.0), pt(60.0, 0.0)),
        ],
        Vec::new(),
        vec![power_label("+3V3", pt(50.0, 0.0))],
        vec![power_port("+3V3", pt(10.0, 0.0))],
    );
    let mut out = Vec::new();
    missing_power_flag(&c, &mut out);
    assert_eq!(
        out.len(),
        1,
        "port on a net with no same-name label is flagged"
    );
}

// -----------------------------------------------------------------------
// Issue #388 regressions — rules must agree with `build_netlist` on
// mid-wire (interior) label/pin placements and through-junction taps.
// -----------------------------------------------------------------------

#[test]
fn unused_pin_trusts_a_through_junction_tap() {
    // A single wire (0,0)-(10,0) with a junction dot mid-span at (5,0) — a
    // pin tapping exactly there is connected (D5.3), the same as
    // `context::point_is_connected` computes. The old `unused_pin` never
    // consulted junctions at all when re-deriving connectivity, so it
    // would have false-flagged this pin.
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![ErcJunction {
            position: pt(5.0, 0.0),
        }],
        Vec::new(),
        vec![symbol_with_pin("R1", pt(5.0, 0.0), true)],
    );
    let mut out = Vec::new();
    unused_pin(&c, &mut out);
    assert!(
        out.is_empty(),
        "a pin tapping a wire interior through a junction is not unused: {out:?}"
    );
}

#[test]
fn unused_pin_still_fires_when_not_connected() {
    // Sanity check: `unused_pin` still reads `connected == false`.
    let c = ctx(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![symbol_with_pin("R1", pt(5.0, 0.0), false)],
    );
    let mut out = Vec::new();
    unused_pin(&c, &mut out);
    assert_eq!(out.len(), 1, "a genuinely unconnected pin is still flagged");
}

#[test]
fn net_label_conflict_catches_wire_interior_labels() {
    // Two conflicting Net labels both sit on the *interior* of the same
    // wire — no endpoints involved. Without anchoring each label to the
    // wire, each lands on its own singleton root and the conflict is
    // missed entirely.
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        Vec::new(),
        vec![net_label("A", pt(3.0, 0.0)), net_label("B", pt(7.0, 0.0))],
        Vec::new(),
    );
    let mut out = Vec::new();
    net_label_conflict(&c, &mut out);
    assert_eq!(
        out.len(),
        1,
        "conflicting Net labels on one wire's interior are flagged: {out:?}"
    );
}

#[test]
fn orphan_label_accepts_a_power_label_on_a_wire_interior() {
    // A Power label sits mid-span, not at either endpoint.
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        Vec::new(),
        vec![power_label("+3V3", pt(5.0, 0.0))],
        Vec::new(),
    );
    let mut out = Vec::new();
    orphan_label(&c, &mut out);
    assert!(
        out.is_empty(),
        "a Power label on a wire interior is not orphaned: {out:?}"
    );
}

#[test]
fn hier_port_disconnected_accepts_a_global_label_on_a_wire_interior() {
    // A Global label sits mid-span, not at either endpoint.
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        Vec::new(),
        vec![global_label("VBUS", pt(5.0, 0.0))],
        Vec::new(),
    );
    let mut out = Vec::new();
    hier_port_disconnected(&c, &mut out);
    assert!(
        out.is_empty(),
        "a Global label on a wire interior is on-wire: {out:?}"
    );
}

#[test]
fn missing_power_flag_accepts_a_mid_wire_cross_ref_label() {
    // The +3V3 label sits mid-span on the same wire the +3V3 port's pin
    // terminates on — connected only via interior anchoring, not a
    // shared endpoint or junction.
    let c = ctx(
        vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
        Vec::new(),
        vec![power_label("+3V3", pt(5.0, 0.0))],
        vec![power_port("+3V3", pt(10.0, 0.0))],
    );
    let mut out = Vec::new();
    missing_power_flag(&c, &mut out);
    assert!(
        out.is_empty(),
        "port cross-referenced by a mid-wire label is not flagged: {out:?}"
    );
}

// -----------------------------------------------------------------------
// Anchor-then-sample must not be order-dependent on label document order
// (follow-up to #388): a wire T where wB ends on wA's interior with NO
// junction. wA = (0,0)-(10,0), wB = (5,0)-(5,10). The T point (5,0) is
// simultaneously wA's interior and wB's own endpoint, so anchoring one
// label there re-roots the whole class — sampling that root before a
// later label's anchoring union used to cache a stale root.
// -----------------------------------------------------------------------

#[test]
fn net_label_conflict_is_independent_of_label_order() {
    let wires = vec![
        wire(pt(0.0, 0.0), pt(10.0, 0.0)),
        wire(pt(5.0, 0.0), pt(5.0, 10.0)),
    ];
    let n1 = net_label("N1", pt(5.0, 5.0)); // wB interior
    let n2 = net_label("N2", pt(5.0, 0.0)); // the T point

    let forward = ctx(
        wires.clone(),
        Vec::new(),
        vec![n1.clone(), n2.clone()],
        Vec::new(),
    );
    let mut out_fwd = Vec::new();
    net_label_conflict(&forward, &mut out_fwd);

    let reverse = ctx(wires, Vec::new(), vec![n2, n1], Vec::new());
    let mut out_rev = Vec::new();
    net_label_conflict(&reverse, &mut out_rev);

    assert_eq!(
        out_fwd.len(),
        1,
        "N1/N2 share a net through the T and conflict: {out_fwd:?}"
    );
    assert_eq!(
        out_fwd.len(),
        out_rev.len(),
        "verdict must not depend on label document order (fwd={out_fwd:?}, rev={out_rev:?})"
    );
}

#[test]
fn missing_power_flag_is_independent_of_label_order() {
    let wires = vec![
        wire(pt(0.0, 0.0), pt(10.0, 0.0)),
        wire(pt(5.0, 0.0), pt(5.0, 10.0)),
    ];
    let power = power_label("+3V3", pt(5.0, 5.0)); // wB interior
    let other = net_label("X", pt(5.0, 0.0)); // the T point
    let port = power_port("+3V3", pt(5.0, 10.0)); // wB's far endpoint

    let forward = ctx(
        wires.clone(),
        Vec::new(),
        vec![power.clone(), other.clone()],
        vec![port.clone()],
    );
    let mut out_fwd = Vec::new();
    missing_power_flag(&forward, &mut out_fwd);

    let reverse = ctx(wires, Vec::new(), vec![other, power], vec![port]);
    let mut out_rev = Vec::new();
    missing_power_flag(&reverse, &mut out_rev);

    assert!(
        out_fwd.is_empty(),
        "port cross-referenced through the T is not flagged: {out_fwd:?}"
    );
    assert_eq!(
        out_fwd.len(),
        out_rev.len(),
        "verdict must not depend on label document order (fwd={out_fwd:?}, rev={out_rev:?})"
    );
}

fn bus_ctx(buses: Vec<crate::context::ErcBus>, labels: Vec<ErcLabel>) -> ErcContext {
    ErcContext {
        buses,
        labels,
        ..ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new())
    }
}

fn bus(a: Point, b: Point) -> crate::context::ErcBus {
    crate::context::ErcBus { start: a, end: b }
}

#[test]
fn bus_bit_width_mismatch_catches_mid_bus_range_labels() {
    // Regression for issue #395: both range labels sit on the bus INTERIOR
    // — where users actually place them — not on an endpoint. Unanchored,
    // each landed on its own singleton root, no group ever reached the
    // `len() >= 2` needed to compare widths, and the mismatch went unsaid.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let labels = vec![
        net_label("D[0..7]", pt(3.0, 0.0)),
        net_label("D[0..3]", pt(7.0, 0.0)),
    ];
    let mut out = Vec::new();
    bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
    assert_eq!(
        out.len(),
        1,
        "a mid-bus width mismatch must be reported: {out:?}"
    );
}

#[test]
fn bus_bit_width_mismatch_still_catches_endpoint_range_labels() {
    // Guard: the case that already worked before #395 must keep working —
    // anchoring an endpoint label is a no-op union.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let labels = vec![
        net_label("D[0..7]", pt(0.0, 0.0)),
        net_label("D[0..3]", pt(10.0, 0.0)),
    ];
    let mut out = Vec::new();
    bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
    assert_eq!(out.len(), 1, "endpoint mismatch still fires: {out:?}");
}

#[test]
fn bus_bit_width_mismatch_accepts_matching_mid_bus_widths() {
    // Anchoring must not turn into a false-positive engine: two mid-span
    // labels that agree on the width are grouped together now (they were
    // two singletons before) and must still report nothing.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let labels = vec![
        net_label("D[0..7]", pt(3.0, 0.0)),
        net_label("D[0..7]", pt(7.0, 0.0)),
    ];
    let mut out = Vec::new();
    bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
    assert!(
        out.is_empty(),
        "matching widths are not a mismatch: {out:?}"
    );
}

#[test]
fn bus_bit_width_mismatch_keeps_separate_buses_apart() {
    // Anchoring is per-segment, not global: two disjoint buses each with
    // their own range label are two groups of one, so differing widths
    // across unrelated buses are not a mismatch.
    let buses = vec![
        bus(pt(0.0, 0.0), pt(10.0, 0.0)),
        bus(pt(0.0, 50.0), pt(10.0, 50.0)),
    ];
    let labels = vec![
        net_label("D[0..7]", pt(5.0, 0.0)),
        net_label("A[0..3]", pt(5.0, 50.0)),
    ];
    let mut out = Vec::new();
    bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
    assert!(
        out.is_empty(),
        "unrelated buses must not be compared: {out:?}"
    );
}

#[test]
fn bus_bit_width_mismatch_ignores_a_label_off_every_bus() {
    // A range label floating off the bus stays its own root and never
    // joins the bundle — anchoring must not attract anything nearby.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let labels = vec![
        net_label("D[0..7]", pt(5.0, 0.0)),
        net_label("D[0..3]", pt(5.0, 25.0)),
    ];
    let mut out = Vec::new();
    bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
    assert!(
        out.is_empty(),
        "an off-bus label must not join the bundle: {out:?}"
    );
}

// ---------------------------------------------------------------------------
// Issue #404 regressions — every rule that groups by net must read the SAME
// topology `build_netlist` derives: physical connectivity *plus* the
// same-name label merge. A rule that merges nothing reports more nets than
// exist; a rule that merges only its own filtered label subset misses joins
// made by the label kinds it filtered out.
// ---------------------------------------------------------------------------

#[test]
fn net_label_conflict_sees_a_join_made_by_a_global_label() {
    // Two disjoint wires. Each carries one Net label and the SAME Global
    // label `VCC`. The Global merge makes them one net, so `build_netlist`
    // returns a single net named `VCC` and BOTH signal names silently vanish
    // — a real conflict the user must be told about. Merging only the
    // Net-filtered subset never sees the join (SIG1 and SIG2 share no text)
    // and reported nothing.
    let c = ctx(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(50.0, 0.0), pt(60.0, 0.0)),
        ],
        Vec::new(),
        vec![
            net_label("SIG1", pt(0.0, 0.0)),
            global_label("VCC", pt(10.0, 0.0)),
            net_label("SIG2", pt(50.0, 0.0)),
            global_label("VCC", pt(60.0, 0.0)),
        ],
        Vec::new(),
    );
    let mut out = Vec::new();
    net_label_conflict(&c, &mut out);
    assert_eq!(
        out.len(),
        1,
        "a Global label joining two differently-named Net labels is a conflict: {out:?}"
    );
    assert!(
        out[0].message.contains("SIG1") && out[0].message.contains("SIG2"),
        "conflict must name both dropped signals: {}",
        out[0].message
    );
}

#[test]
fn missing_power_flag_honors_a_third_label_name_merge() {
    // Port `+3V3` sits on wire A, which carries no `+3V3` label of its own.
    // A Net label `VDD` on wire A and another on wire B merge the two into
    // one net, and wire B carries the `+3V3` label. The port IS therefore
    // cross-referenced on its real net → no diagnostic. Without the
    // same-name merge the two wires stay separate and this false-positived.
    let c = ctx(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(50.0, 0.0), pt(60.0, 0.0)),
        ],
        Vec::new(),
        vec![
            net_label("VDD", pt(0.0, 0.0)),
            net_label("VDD", pt(50.0, 0.0)),
            power_label("+3V3", pt(60.0, 0.0)),
        ],
        vec![power_port("+3V3", pt(10.0, 0.0))],
    );
    let mut out = Vec::new();
    missing_power_flag(&c, &mut out);
    assert!(
        out.is_empty(),
        "port cross-referenced through a third label's name merge: {out:?}"
    );
}

#[test]
fn missing_power_flag_still_fires_on_a_bare_net_beside_a_merged_pair() {
    // MD-12 guard against over-merging: the port's own wire is bare. Two OTHER
    // wires each carry `+3V3` and merge with each other by name — that merge
    // must not drag the port's unrelated net in. The port is still uncrossed
    // and must be flagged exactly once.
    let c = ctx(
        vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(50.0, 0.0), pt(60.0, 0.0)),
            wire(pt(100.0, 0.0), pt(110.0, 0.0)),
        ],
        Vec::new(),
        vec![
            power_label("+3V3", pt(50.0, 0.0)),
            power_label("+3V3", pt(100.0, 0.0)),
        ],
        vec![power_port("+3V3", pt(10.0, 0.0))],
    );
    let mut out = Vec::new();
    missing_power_flag(&c, &mut out);
    assert_eq!(
        out.len(),
        1,
        "a name merge elsewhere must not suppress a bare port's net: {out:?}"
    );
}
