use super::*;

// ---------------------------------------------------------------------------
// Issue #1048 — bus members: naming, entry wires, shared parser, anchoring.
// ---------------------------------------------------------------------------

/// Vertical bus (0,0)-(0,20) with the given labels, one entry at (0,5) whose
/// stub ends at (2.54, 7.54), and a wire from there to (12.54, 7.54).
fn entry_ctx(labels: Vec<ErcLabel>) -> ErcContext {
    ErcContext {
        buses: vec![crate::context::ErcBus {
            uuid: Uuid::nil(),
            start: pt(0.0, 0.0),
            end: pt(0.0, 20.0),
        }],
        bus_entries: vec![crate::context::ErcBusEntry {
            uuid: Uuid::nil(),
            position: pt(0.0, 5.0),
            size: (2.54, 2.54),
        }],
        wires: vec![wire(pt(2.54, 7.54), pt(12.54, 7.54))],
        labels,
        ..ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new())
    }
}

fn run_bus_rules(c: &ErcContext) -> Vec<crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    let bus = bus_analysis(c);
    bus_entry_unlabeled(c, &bus, &mut out);
    bus_member_not_in_bus(c, &bus, &mut out);
    bus_unnamed(c, &bus, &mut out);
    bus_name_conflict(c, &bus, &mut out);
    bus_bit_width_mismatch(c, &bus, &mut out);
    bus_label_on_wire(c, &bus, &mut out);
    out
}

fn kinds(out: &[crate::diagnostic::Diagnostic]) -> Vec<crate::RuleKind> {
    out.iter().map(|d| d.rule_kind).collect()
}

#[test]
fn a_labeled_entry_wire_with_a_member_label_is_clean() {
    let c = entry_ctx(vec![
        net_label("D[0..7]", pt(0.0, 10.0)),
        net_label("D3", pt(6.0, 7.54)),
    ]);
    assert!(run_bus_rules(&c).is_empty());
}

#[test]
fn an_entry_wire_without_a_label_warns() {
    let c = entry_ctx(vec![net_label("D[0..7]", pt(0.0, 10.0))]);
    assert_eq!(
        kinds(&run_bus_rules(&c)),
        [crate::RuleKind::BusEntryUnlabeled]
    );
}

#[test]
fn a_member_label_outside_the_bus_range_warns() {
    for typo in ["D33", "D8", "D3 ", "d3", "D03"] {
        let c = entry_ctx(vec![
            net_label("D[0..7]", pt(0.0, 10.0)),
            net_label(typo, pt(6.0, 7.54)),
        ]);
        assert_eq!(
            kinds(&run_bus_rules(&c)),
            [crate::RuleKind::BusMemberNotInBus],
            "{typo:?}"
        );
    }
}

#[test]
fn group_buses_define_their_own_members() {
    let group = entry_ctx(vec![
        net_label("{SDA SCL}", pt(0.0, 10.0)),
        net_label("SCL", pt(6.0, 7.54)),
    ]);
    assert!(run_bus_rules(&group).is_empty());
    let wrong = entry_ctx(vec![
        net_label("{SDA SCL}", pt(0.0, 10.0)),
        net_label("SDA2", pt(6.0, 7.54)),
    ]);
    assert_eq!(
        kinds(&run_bus_rules(&wrong)),
        [crate::RuleKind::BusMemberNotInBus]
    );
}

#[test]
fn a_bus_with_no_name_warns_once_and_hides_member_checks() {
    let c = entry_ctx(vec![net_label("D3", pt(6.0, 7.54))]);
    assert_eq!(kinds(&run_bus_rules(&c)), [crate::RuleKind::BusUnnamed]);
}

#[test]
fn a_bus_labelled_with_a_non_bus_name_is_still_unnamed() {
    let c = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![net_label("VCC", pt(0.0, 0.0))],
    );
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusUnnamed]);
    assert!(out[0].message.contains("VCC"));
}

#[test]
fn width_mismatch_uses_the_shared_parser_for_every_syntax() {
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let count = |a: &str, b: &str| {
        let labels = vec![net_label(a, pt(3.0, 0.0)), net_label(b, pt(7.0, 0.0))];
        let c = bus_ctx(buses.clone(), labels);
        let (mut width, mut conflict) = (Vec::new(), Vec::new());
        let bus = bus_analysis(&c);
        bus_bit_width_mismatch(&c, &bus, &mut width);
        bus_name_conflict(&c, &bus, &mut conflict);
        (width.len(), conflict.len())
    };
    assert_eq!(
        count("D[7:0]", "D[0..7]"),
        (0, 0),
        "colon form and direction"
    );
    assert_eq!(
        count("{SDA SCL}", "{SCL SDA}"),
        (0, 0),
        "same set, any order"
    );
    assert_eq!(count("{SDA SCL}", "D[0..3]"), (1, 0));
    assert_eq!(count("D[0..3]", "D[0..7]"), (1, 0));
}

#[test]
fn two_different_names_on_one_run_are_a_name_conflict() {
    // Equal width, different signals: the width rule is silent, the name
    // rule is not.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    for (a, b) in [
        ("D[0..7]", "D[8..15]"),
        ("{SDA SCL}", "I[0..1]"),
        ("D[7..0]", "A[0..7]"),
    ] {
        let labels = vec![net_label(a, pt(3.0, 0.0)), net_label(b, pt(7.0, 0.0))];
        let c = bus_ctx(buses.clone(), labels);
        let bus = bus_analysis(&c);
        let (mut width, mut conflict) = (Vec::new(), Vec::new());
        bus_bit_width_mismatch(&c, &bus, &mut width);
        bus_name_conflict(&c, &bus, &mut conflict);
        assert!(width.is_empty(), "{a} vs {b}");
        assert_eq!(conflict.len(), 1, "{a} vs {b}");
        assert_eq!(conflict[0].rule_kind, crate::RuleKind::BusNameConflict);
    }
}

#[test]
fn a_t_branched_bus_is_one_named_run() {
    let mut c = bus_ctx(
        vec![
            bus(pt(0.0, 0.0), pt(0.0, 20.0)),
            bus(pt(0.0, 10.0), pt(10.0, 10.0)),
        ],
        vec![net_label("D[0..7]", pt(0.0, 3.0))],
    );
    c.junctions.push(crate::context::ErcJunction {
        position: pt(0.0, 10.0),
    });
    assert!(run_bus_rules(&c).is_empty(), "{:?}", run_bus_rules(&c));
}

#[test]
fn a_malformed_bus_label_warns_even_when_the_bus_has_another_valid_name() {
    let c = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![
            net_label("D[0..7]", pt(3.0, 0.0)),
            net_label("D[0 .. 7]", pt(0.0, 0.0)),
            net_label("VCC", pt(10.0, 0.0)),
        ],
    );
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusUnnamed]);
    assert!(out[0].message.contains("D[0 .. 7]"));
    assert!(!out[0].message.contains("carries no members"));
}

#[test]
fn a_bus_name_label_mid_bus_is_anchored_for_every_rule() {
    // One anchoring rule: a bus-name label touches the bus anywhere along it.
    // It used to be orphaned (endpoint-only) while the width rule grouped it.
    let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
    let mut global = net_label("D[0..7]", pt(5.0, 0.0));
    global.label_type = LabelType::Global;
    let c = bus_ctx(buses.clone(), vec![global]);
    let mut out = Vec::new();
    hier_port_disconnected(&c, &mut out);
    assert!(out.is_empty(), "mid-bus global bus label is on the bus");

    let c = bus_ctx(buses.clone(), vec![net_label("D[0..7]", pt(5.0, 0.0))]);
    let mut out = Vec::new();
    orphan_label(&c, &mut out);
    assert!(out.is_empty(), "mid-bus net bus label is on the bus");

    // A non-bus label keeps endpoint-only contact with a bus (D5.4).
    let c = bus_ctx(buses, vec![net_label("CLK", pt(5.0, 0.0))]);
    let mut out = Vec::new();
    orphan_label(&c, &mut out);
    assert_eq!(out.len(), 1, "a plain net label mid-bus is still orphaned");
}

fn big_sheet(n: usize) -> ErcContext {
    let mut buses = Vec::new();
    let mut labels = Vec::new();
    for i in 0..n {
        let y = 30.0 * i as f64;
        buses.push(bus(pt(0.0, y), pt(0.0, y + 20.0)));
        labels.push(net_label("D[0..7]", pt(0.0, y + 15.0)));
    }
    bus_ctx(buses, labels)
}

#[test]
fn a_big_sheet_is_clean_and_an_oversized_label_is_refused() {
    let mut c = big_sheet(2000);
    assert!(run_bus_rules(&c).is_empty());
    // 40 x 4096 signals: refused by the parser before any expansion, so the
    // run is left without a valid name and the label is reported as invalid.
    c.labels[0].text = format!("{{{}}}", ["A[0..4095]"; 40].join(" "));
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusUnnamed]);
}

/// Wall-clock check; run with
/// `cargo test -p signex-erc -- --ignored big_sheet_benchmark`.
#[test]
#[ignore = "benchmark"]
fn big_sheet_benchmark() {
    let c = big_sheet(20_000);
    let start = std::time::Instant::now();
    assert!(run_bus_rules(&c).is_empty());
    eprintln!("20000 runs checked in {:?}", start.elapsed());
}

#[test]
fn a_malformed_bus_label_mid_span_is_reported_once_and_is_not_an_orphan() {
    // `D[0 .. 7]` in the middle of the bus: one typo, one diagnostic.
    let c = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![
            net_label("D[0..7]", pt(0.0, 0.0)),
            net_label("D[0 .. 7]", pt(5.0, 0.0)),
        ],
    );
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusUnnamed]);
    assert!(out[0].message.contains("D[0 .. 7]"));
    let mut orphans = Vec::new();
    orphan_label(&c, &mut orphans);
    assert!(
        orphans.is_empty(),
        "already reported as a malformed bus label"
    );
}

#[test]
fn a_plain_net_name_mid_bus_is_neither_reported_as_malformed_nor_on_the_bus() {
    let c = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![
            net_label("D[0..7]", pt(0.0, 0.0)),
            net_label("CLK", pt(5.0, 0.0)),
        ],
    );
    assert!(run_bus_rules(&c).is_empty());
    let mut orphans = Vec::new();
    orphan_label(&c, &mut orphans);
    assert_eq!(orphans.len(), 1);
}

#[test]
fn a_bus_name_label_on_a_wire_warns_but_changes_no_connectivity() {
    let mut c = ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    c.labels.push(net_label("D[0..7]", pt(5.0, 0.0)));
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusLabelOnWire]);
    assert!(out[0].message.contains("D[0..7]"));
}

#[test]
fn a_bus_name_label_on_a_bus_or_floating_is_not_a_label_on_a_wire() {
    let on_bus = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![net_label("D[0..7]", pt(5.0, 0.0))],
    );
    assert!(run_bus_rules(&on_bus).is_empty());
    let floating = bus_ctx(Vec::new(), vec![net_label("D[0..7]", pt(5.0, 50.0))]);
    assert!(
        run_bus_rules(&floating).is_empty(),
        "orphan_label covers it"
    );
}

#[test]
fn a_wire_fed_by_two_entries_warns_once_per_label() {
    let mut c = entry_ctx(vec![
        net_label("D[0..7]", pt(0.0, 10.0)),
        net_label("D33", pt(6.0, 7.54)),
    ]);
    // A second stub into the same wire, from the same bus.
    c.bus_entries.push(crate::context::ErcBusEntry {
        uuid: Uuid::nil(),
        position: pt(0.0, 3.0),
        size: (2.54, 4.54),
    });
    let out = run_bus_rules(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusMemberNotInBus]);
}
