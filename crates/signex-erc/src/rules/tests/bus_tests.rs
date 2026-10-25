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
fn the_bus_rules_stay_fast_on_a_big_sheet() {
    let n = 2000;
    let mut buses = Vec::new();
    let mut labels = Vec::new();
    for i in 0..n {
        let y = 30.0 * i as f64;
        buses.push(bus(pt(0.0, y), pt(0.0, y + 20.0)));
        labels.push(net_label("D[0..7]", pt(0.0, y + 15.0)));
    }
    let mut c = bus_ctx(buses, labels);
    let start = std::time::Instant::now();
    assert!(run_bus_rules(&c).is_empty());
    c.labels[0].text = format!("{{{}}}", ["A[0..4095]"; 40].join(" "));
    assert_eq!(run_bus_rules(&c).len(), 1, "the oversized label is refused");
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
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
